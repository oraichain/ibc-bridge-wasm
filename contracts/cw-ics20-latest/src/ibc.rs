use std::ops::Mul;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, coin, entry_point, from_binary, to_binary, Addr, Api, Binary, CosmosMsg, Decimal, Deps,
    DepsMut, Env, IbcBasicResponse, IbcChannel, IbcChannelCloseMsg, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcEndpoint, IbcMsg, IbcOrder, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, IbcTimeout, Order,
    QuerierWrapper, Reply, Response, StdError, StdResult, Storage, SubMsg, SubMsgResult, Timestamp,
    Uint128,
};
use cw20_ics20_msg::helper::{
    denom_to_asset_info, get_prefix_decode_bech32, parse_asset_info_denom,
};
use cw20_ics20_msg::receiver::DestinationInfo;
use cw_storage_plus::Map;
use oraiswap::asset::AssetInfo;
use oraiswap::router::{RouterController, SwapOperation};

use crate::error::{ContractError, Never};
use crate::state::{
    get_key_ics20_ibc_denom, ics20_denoms, increase_channel_balance, reduce_channel_balance,
    undo_reduce_channel_balance, ChannelInfo, MappingMetadata, Ratio, ALLOW_LIST, CHANNEL_INFO,
    CONFIG, RELAYER_FEE, RELAYER_FEE_ACCUMULATOR, TOKEN_FEE, TOKEN_FEE_ACCUMULATOR,
};
use cw20_ics20_msg::amount::{convert_local_to_remote, convert_remote_to_local, Amount};

pub const ICS20_VERSION: &str = "ics20-1";
pub const ICS20_ORDERING: IbcOrder = IbcOrder::Unordered;
pub const ORAIBRIDGE_PREFIX: &str = "oraib";

/// The format for sending an ics20 packet.
/// Proto defined here: https://github.com/cosmos/cosmos-sdk/blob/v0.42.0/proto/ibc/applications/transfer/v1/transfer.proto#L11-L20
/// This is compatible with the JSON serialization
#[cw_serde]
pub struct Ics20Packet {
    /// amount of tokens to transfer is encoded as a string, but limited to u64 max
    pub amount: Uint128,
    /// the token denomination to be transferred
    pub denom: String,
    /// the recipient address on the destination chain
    pub receiver: String,
    /// the sender address
    pub sender: String,
    /// optional memo
    pub memo: Option<String>,
}

impl Ics20Packet {
    pub fn new<T: Into<String>>(
        amount: Uint128,
        denom: T,
        sender: &str,
        receiver: &str,
        memo: Option<String>,
    ) -> Self {
        Ics20Packet {
            denom: denom.into(),
            amount,
            sender: sender.to_string(),
            receiver: receiver.to_string(),
            memo,
        }
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.amount.u128() > (u128::MAX as u128) {
            Err(ContractError::AmountOverflow {})
        } else {
            Ok(())
        }
    }
}

/// This is a generic ICS acknowledgement format.
/// Proto defined here: https://github.com/cosmos/cosmos-sdk/blob/v0.42.0/proto/ibc/core/channel/v1/channel.proto#L141-L147
/// This is compatible with the JSON serialization
#[cw_serde]
pub enum Ics20Ack {
    Result(Binary),
    Error(String),
}

// create a serialized success message
fn ack_success() -> Binary {
    let res = Ics20Ack::Result(b"1".into());
    to_binary(&res).unwrap()
}

// create a serialized error message
pub fn ack_fail(err: String) -> Binary {
    let res = Ics20Ack::Error(err);
    to_binary(&res).unwrap()
}

pub const ACK_FAILURE_ID: u64 = 64023;

#[entry_point]
pub fn reply(_deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        ACK_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => Ok(Response::new().set_data(ack_fail(err))),
        },
        _ => Err(ContractError::UnknownReplyId { id: reply.id }),
    }
}

#[entry_point]
/// enforces ordering and versioning constraints
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> Result<(), ContractError> {
    enforce_order_and_version(msg.channel(), msg.counterparty_version())?;
    Ok(())
}

#[entry_point]
/// record the channel in CHANNEL_INFO
pub fn ibc_channel_connect(
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // we need to check the counter party version in try and ack (sometimes here)
    enforce_order_and_version(msg.channel(), msg.counterparty_version())?;

    let channel: IbcChannel = msg.into();
    let info = ChannelInfo {
        id: channel.endpoint.channel_id,
        counterparty_endpoint: channel.counterparty_endpoint,
        connection_id: channel.connection_id,
    };
    CHANNEL_INFO.save(deps.storage, &info.id, &info)?;

    Ok(IbcBasicResponse::default())
}

fn enforce_order_and_version(
    channel: &IbcChannel,
    counterparty_version: Option<&str>,
) -> Result<(), ContractError> {
    if channel.version != ICS20_VERSION {
        return Err(ContractError::InvalidIbcVersion {
            version: channel.version.clone(),
        });
    }
    if let Some(version) = counterparty_version {
        if version != ICS20_VERSION {
            return Err(ContractError::InvalidIbcVersion {
                version: version.to_string(),
            });
        }
    }
    if channel.order != ICS20_ORDERING {
        return Err(ContractError::OnlyOrderedChannel {});
    }
    Ok(())
}

#[entry_point]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    _channel: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // TODO: what to do here?
    // we will have locked funds that need to be returned somehow
    unimplemented!();
}

#[entry_point]
/// Check to see if we have any balance here
/// We should not return an error if possible, but rather an acknowledgement of failure
pub fn ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    let packet = msg.packet;

    do_ibc_packet_receive(deps, env, &packet).or_else(|err| {
        Ok(IbcReceiveResponse::new()
            .set_ack(ack_fail(err.to_string()))
            .add_attributes(vec![
                attr("action", "receive"),
                attr("success", "false"),
                attr("error", err.to_string()),
            ]))
    })
}

// Returns local denom if the denom is an encoded voucher from the expected endpoint
// Otherwise, error
pub fn parse_voucher_denom<'a>(
    voucher_denom: &'a str,
    remote_endpoint: &IbcEndpoint,
) -> Result<(&'a str, bool), ContractError> {
    let split_denom: Vec<&str> = voucher_denom.splitn(3, '/').collect();

    // if it is a packet_receive of native token from chain A or IBC token that was sent from chain B.
    if split_denom.len() == 1 {
        return Ok((voucher_denom, true));
    }
    if split_denom.len() != 3 {
        return Err(ContractError::NoForeignTokens {});
    }
    // a few more sanity checks
    if split_denom[0] != remote_endpoint.port_id {
        return Err(ContractError::FromOtherPort {
            port: split_denom[0].into(),
        });
    }
    if split_denom[1] != remote_endpoint.channel_id {
        return Err(ContractError::FromOtherChannel {
            channel: split_denom[1].into(),
        });
    }

    Ok((split_denom[2], false))
}

// Returns local denom if the denom is an encoded voucher from the expected endpoint
// Otherwise, error
pub fn parse_ibc_denom_without_sanity_checks<'a>(ibc_denom: &'a str) -> StdResult<&'a str> {
    let split_denom: Vec<&str> = ibc_denom.splitn(3, '/').collect();

    if split_denom.len() != 3 {
        return Err(StdError::generic_err(
            ContractError::NoForeignTokens {}.to_string(),
        ));
    }
    Ok(split_denom[2])
}

// Returns
// Otherwise, error
pub fn parse_ibc_channel_without_sanity_checks<'a>(ibc_denom: &'a str) -> StdResult<&'a str> {
    let split_denom: Vec<&str> = ibc_denom.splitn(3, '/').collect();

    if split_denom.len() != 3 {
        return Err(StdError::generic_err(
            ContractError::NoForeignTokens {}.to_string(),
        ));
    }
    Ok(split_denom[1])
}

// this does the work of ibc_packet_receive, we wrap it to turn errors into acknowledgements
fn do_ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    packet: &IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;
    // let channel = packet.dest.channel_id.clone();

    // If the token originated on the remote chain, it looks like "ucosm".
    // If it originated on our chain, it looks like "port/channel/ucosm".
    let denom = parse_voucher_denom(&msg.denom, &packet.src)?;

    // if denom is native, we handle it the native way
    if denom.1 {
        return handle_ibc_packet_receive_native_remote_chain(
            deps.storage,
            deps.api,
            &deps.querier,
            env,
            &denom.0,
            &packet,
            &msg,
        );
    }

    Err(ContractError::Std(StdError::generic_err("Not suppported")))
}

fn handle_ibc_packet_receive_native_remote_chain(
    storage: &mut dyn Storage,
    api: &dyn Api,
    querier: &QuerierWrapper,
    env: Env,
    denom: &str,
    packet: &IbcPacket,
    msg: &Ics20Packet,
) -> Result<IbcReceiveResponse, ContractError> {
    let config = CONFIG.load(storage)?;

    // key in form transfer/channel-0/foo
    let ibc_denom = get_key_ics20_ibc_denom(&packet.dest.port_id, &packet.dest.channel_id, denom);
    let pair_mapping = ics20_denoms()
        .load(storage, &ibc_denom)
        .map_err(|_| ContractError::NotOnMappingList {})?;

    let to_send = Amount::from_parts(
        parse_asset_info_denom(pair_mapping.asset_info.clone()),
        convert_remote_to_local(
            msg.amount,
            pair_mapping.remote_decimals,
            pair_mapping.asset_info_decimals,
        )?,
    );
    // will have to increase balance here because if this tx fails then it will be reverted, and the balance on the remote chain will also be reverted
    increase_channel_balance(
        storage,
        &packet.dest.channel_id,
        &ibc_denom,
        msg.amount.clone(),
        false,
    )?;

    let (new_deducted_amount, token_fee, relayer_fee) = process_deduct_fee(
        storage,
        querier,
        api,
        &msg.sender,
        &msg.denom,
        to_send.clone(),
        pair_mapping.asset_info_decimals,
        &config.swap_router_contract,
    )?;
    let new_deducted_to_send = Amount::from_parts(to_send.denom(), new_deducted_amount);

    // after receiving the cw20 amount, we try to do fee swapping for the user if needed so he / she can create txs on the network
    let (cosmos_msgs, ibc_error_msg) = get_follow_up_msgs(
        storage,
        api,
        querier,
        env.clone(),
        new_deducted_to_send,
        pair_mapping.asset_info,
        &msg.sender,
        &msg.receiver,
        &msg.memo.clone().unwrap_or_default(),
        packet.dest.channel_id.as_str(),
    )?;
    let mut fee_msgs = collect_fee_msgs(
        storage,
        config.token_fee_receiver.into_string(),
        TOKEN_FEE_ACCUMULATOR,
    )?;
    fee_msgs.append(&mut collect_fee_msgs(
        storage,
        config.relayer_fee_receiver.to_string(),
        RELAYER_FEE_ACCUMULATOR,
    )?);
    let mut res = IbcReceiveResponse::new()
        .set_ack(ack_success())
        .add_messages(fee_msgs) // if one of messages fail, the entire tx will be reverted
        .add_messages(cosmos_msgs)
        .add_attribute("action", "receive_native")
        .add_attribute("sender", msg.sender.clone())
        .add_attribute("receiver", msg.receiver.clone())
        .add_attribute("denom", denom)
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("success", "true")
        .add_attribute("token_fee", token_fee)
        .add_attribute("relayer_fee", relayer_fee);
    if !ibc_error_msg.is_empty() {
        res = res.add_attribute("ibc_error_msg", ibc_error_msg);
    }

    Ok(res)
}

pub fn get_follow_up_msgs(
    storage: &mut dyn Storage,
    api: &dyn Api,
    querier: &QuerierWrapper,
    env: Env,
    to_send: Amount,
    initial_receive_asset_info: AssetInfo,
    sender: &str,
    receiver: &str,
    memo: &str,
    initial_dest_channel_id: &str, // channel id on Oraichain receiving the token from other chain
) -> Result<(Vec<CosmosMsg>, String), ContractError> {
    let config = CONFIG.load(storage)?;
    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
    let destination: DestinationInfo = DestinationInfo::from_str(memo);
    let send_only_sub_msg = to_send.send_amount(receiver.to_string(), None);
    if is_follow_up_msgs_only_send_amount(&memo, &destination.destination_denom) {
        return Ok((vec![send_only_sub_msg], "".to_string()));
    }
    // successful case. We dont care if this msg is going to be successful or not because it does not affect our ibc receive flow (just submsgs)
    let receiver_asset_info = denom_to_asset_info(querier, api, &destination.destination_denom)?;
    let swap_operations = build_swap_operations(
        receiver_asset_info.clone(),
        initial_receive_asset_info.clone(),
        config.fee_denom.as_str(),
    );
    let mut minimum_receive = to_send.amount();
    if swap_operations.len() > 0 {
        let response = config.swap_router_contract.simulate_swap(
            querier,
            to_send.amount().clone(),
            swap_operations.clone(),
        );
        if response.is_err() {
            return Ok((
                vec![send_only_sub_msg],
                format!(
                    "Cannot simulate swap with ops: {:?} with error: {:?}",
                    swap_operations,
                    response.unwrap_err().to_string()
                ),
            ));
        }
        minimum_receive = response.unwrap().amount;
    }

    let ibc_msg = build_ibc_msg(
        storage,
        env,
        receiver_asset_info,
        initial_dest_channel_id,
        minimum_receive.clone(),
        &sender,
        &destination,
        config.default_timeout,
    );

    // by default, the receiver is the original address sent in ics20packet
    let mut to = Some(api.addr_validate(receiver)?);
    let ibc_error_msg = if let Some(ibc_msg) = ibc_msg.as_ref().ok() {
        cosmos_msgs.push(ibc_msg.to_owned());
        // if there's an ibc msg => swap receiver is None so the receiver is this ibc wasm address
        to = None;
        String::from("")
    } else {
        ibc_msg.unwrap_err().to_string()
    };
    build_swap_msgs(
        minimum_receive,
        &config.swap_router_contract,
        to_send.amount(),
        initial_receive_asset_info,
        to,
        &mut cosmos_msgs,
        swap_operations,
    )?;
    // fallback case. If there's no cosmos messages or ibc error msg is not empty then we return send amount
    if cosmos_msgs.is_empty() {
        return Ok((vec![send_only_sub_msg], ibc_error_msg));
    }
    return Ok((cosmos_msgs, ibc_error_msg));
}

pub fn is_follow_up_msgs_only_send_amount(memo: &str, destination_denom: &str) -> bool {
    if memo.is_empty() {
        return true;
    }
    // if destination denom, then we simply transfers cw20 to the receiver address.
    if destination_denom.is_empty() {
        return true;
    }
    false
}

pub fn build_swap_operations(
    receiver_asset_info: AssetInfo,
    initial_receive_asset_info: AssetInfo,
    fee_denom: &str,
) -> Vec<SwapOperation> {
    // always swap with orai first cuz its base denom & every token has a pair with it
    let fee_denom_asset_info = AssetInfo::NativeToken {
        denom: fee_denom.to_string(),
    };
    let mut swap_operations = vec![];
    if receiver_asset_info.eq(&initial_receive_asset_info) {
        return vec![];
    }
    if initial_receive_asset_info.ne(&fee_denom_asset_info) {
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: initial_receive_asset_info.clone(),
            ask_asset_info: fee_denom_asset_info.clone(),
        })
    }
    if receiver_asset_info.to_string().ne(fee_denom) {
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: fee_denom_asset_info.clone(),
            ask_asset_info: receiver_asset_info,
        });
    }
    swap_operations
}

pub fn build_swap_msgs(
    minimum_receive: Uint128,
    swap_router_contract: &RouterController,
    amount: Uint128,
    initial_receive_asset_info: AssetInfo,
    to: Option<Addr>,
    sub_msgs: &mut Vec<CosmosMsg>,
    operations: Vec<SwapOperation>,
) -> StdResult<()> {
    // the swap msg must be executed before other msgs because we need the ask token amount to create ibc msg => insert in first index
    if operations.len() == 0 {
        return Ok(());
    }
    sub_msgs.insert(
        0,
        swap_router_contract.execute_operations(
            initial_receive_asset_info,
            amount,
            operations,
            Some(minimum_receive),
            to,
        )?,
    );

    Ok(())
}

pub fn build_ibc_msg(
    storage: &mut dyn Storage,
    env: Env,
    receiver_asset_info: AssetInfo,
    local_channel_id: &str,
    amount: Uint128,
    remote_address: &str,
    destination: &DestinationInfo,
    default_timeout: u64,
) -> StdResult<CosmosMsg> {
    // if there's no dest channel then we stop, no need to transfer ibc
    if destination.destination_channel.is_empty() {
        return Err(StdError::generic_err(
            "Destination channel empty in build ibc msg",
        ));
    }
    let timeout = env.block.time.plus_seconds(default_timeout);
    let pair_mappings: Vec<(String, MappingMetadata)> = ics20_denoms()
        .idx
        .asset_info
        .prefix(receiver_asset_info.to_string())
        .range(storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<(String, MappingMetadata)>>>()?;

    let (is_evm_based, evm_prefix) = destination.is_receiver_evm_based();
    if is_evm_based {
        let mapping = pair_mappings
            .into_iter()
            .find(|(key, _)| {
                // eg: 'wasm.orai195269awwnt5m6c843q6w7hp8rt0k7syfu9de4h0wz384slshuzps8y7ccm/channel-29/eth-mainnet0x4c11249814f11b9346808179Cf06e71ac328c1b5'
                // parse to get eth-mainnet0x...
                // then collect eth-mainnet prefix, and compare with dest channel
                // then we compare the dest channel with the pair mapping. They should match as well
                convert_remote_denom_to_evm_prefix(
                    parse_ibc_denom_without_sanity_checks(key).unwrap_or_default(),
                )
                .eq(&evm_prefix)
                    && parse_ibc_channel_without_sanity_checks(key)
                        .unwrap_or_default()
                        .eq(&destination.destination_channel)
            })
            .ok_or(StdError::generic_err("cannot find pair mappings"))?;
        return process_ibc_msg(
            storage,
            mapping,
            receiver_asset_info,
            local_channel_id,
            env.contract.address.as_str(),
            remote_address, // use sender from ICS20Packet as receiver when transferring back because we have the actual receiver in memo for evm cases
            Some(destination.receiver.clone()),
            amount,
            timeout,
        );
    }
    // 2nd case, where destination network is not evm, but it is still supported on our channel (eg: cw20 ATOM mapped with native ATOM on Cosmos), then we call
    let is_cosmos_based = destination.is_receiver_cosmos_based();
    if is_cosmos_based {
        // eg: wasm.orai195269awwnt5m6c843q6w7hp8rt0k7syfu9de4h0wz384slshuzps8y7ccm/channel-124/uatom
        // for cosmos-based networks, each will have its own channel id => we filter using channel id, no need to check for denom
        let mapping = pair_mappings.into_iter().find(|(key, _)| {
            parse_ibc_channel_without_sanity_checks(key)
                .unwrap_or_default()
                .eq(&destination.destination_channel)
        });
        if let Some(mapping) = mapping {
            return process_ibc_msg(
                storage,
                mapping,
                receiver_asset_info,
                &destination.destination_channel,
                env.contract.address.as_str(),
                &destination.receiver, // now we use dest receiver since cosmos based universal swap wont be sent to oraibridge, so the receiver is the correct receive addr
                None, // no need memo because it is not used in the remote cosmos based chain
                amount,
                timeout,
            );
        }

        // final case, where the destination token is from a remote chain that we dont have a pair mapping with.
        // we use ibc transfer so that attackers cannot manipulate the data to send to oraibridge without reducing the channel balance
        // by using ibc transfer, the contract must actually owns native ibc tokens, which is not possible if it's oraibridge tokens
        // we do not need to reduce channel balance because this transfer is not on our contract channel, but on destination channel
        let ibc_msg: CosmosMsg = IbcMsg::Transfer {
            channel_id: destination.destination_channel.clone(),
            to_address: destination.receiver.clone(),
            amount: coin(amount.u128(), destination.destination_denom.clone()),
            timeout: timeout.into(),
        }
        .into();
        return Ok(ibc_msg);
    }
    Err(StdError::generic_err(
        "The destination info is neither evm or cosmos based",
    ))
}

// TODO: Write unit tests for relayer fee & cosmos based universal swap in simulate js
pub fn process_ibc_msg(
    storage: &mut dyn Storage,
    pair_mapping: (String, MappingMetadata),
    receiver_asset_info: AssetInfo,
    src_channel: &str,
    ibc_msg_sender: &str,
    ibc_msg_receiver: &str,
    memo: Option<String>,
    amount: Uint128,
    timeout: Timestamp,
) -> StdResult<CosmosMsg> {
    let (new_deducted_amount, _) = deduct_token_fee(
        storage,
        parse_ibc_denom_without_sanity_checks(&pair_mapping.0)?, // denom mapping in the form port/channel/denom
        amount,
        &parse_asset_info_denom(receiver_asset_info.clone()),
    )?;
    let remote_amount = convert_local_to_remote(
        new_deducted_amount,
        pair_mapping.1.remote_decimals,
        pair_mapping.1.asset_info_decimals,
    )?;

    // because we are transferring back, we reduce the channel's balance
    reduce_channel_balance(
        storage,
        src_channel.clone(),
        &pair_mapping.0.clone(),
        remote_amount,
        false,
    )
    .map_err(|err| StdError::generic_err(err.to_string()))?;

    // prepare ibc message
    let msg: CosmosMsg = build_ibc_send_packet(
        remote_amount,
        &pair_mapping.0,
        ibc_msg_sender,
        ibc_msg_receiver,
        memo,
        src_channel,
        timeout.into(),
    )?
    .into();

    Ok(msg)
}
pub fn check_gas_limit(deps: Deps, amount: &Amount) -> Result<Option<u64>, ContractError> {
    match amount {
        Amount::Cw20(coin) => {
            // if cw20 token, use the registered gas limit, or error if not whitelisted
            let addr = deps.api.addr_validate(&coin.address)?;
            let allowed = ALLOW_LIST.may_load(deps.storage, &addr)?;
            match allowed {
                Some(allow) => Ok(allow.gas_limit),
                None => match CONFIG.load(deps.storage)?.default_gas_limit {
                    Some(base) => Ok(Some(base)),
                    None => Err(ContractError::NotOnAllowList),
                },
            }
        }
        _ => Ok(None),
    }
}

pub fn process_deduct_fee(
    storage: &mut dyn Storage,
    querier: &QuerierWrapper,
    api: &dyn Api,
    remote_sender: &str,
    remote_token_denom: &str,
    local_amount: Amount, // local amount
    decimals: u8,
    swap_router_contract: &RouterController,
) -> StdResult<(Uint128, Uint128, Uint128)> {
    let (_, token_fee) = deduct_token_fee(
        storage,
        remote_token_denom,
        local_amount.amount(),
        &local_amount.denom(),
    )?;
    // simulate for relayer fee
    let offer_asset_info = denom_to_asset_info(querier, api, &local_amount.raw_denom())?;
    let offer_amount = Uint128::from(10u64.pow((decimals + 1) as u32) as u64); // +1 to make sure the offer amount is large enough to swap successfully
    let token_price = swap_router_contract
        .simulate_swap(
            querier,
            offer_amount,
            vec![SwapOperation::OraiSwap {
                offer_asset_info,
                // always swap with orai. If it does not share a pool with ORAI => ignore, no fee
                ask_asset_info: AssetInfo::NativeToken {
                    denom: "orai".to_string(),
                },
            }],
        )
        .map(|data| data.amount)
        .unwrap_or_default();
    let (_, relayer_fee) = deduct_relayer_fee(
        storage,
        api,
        remote_sender,
        remote_token_denom,
        local_amount.amount(),
        offer_amount,
        &local_amount.denom(),
        token_price,
    )?;
    let new_amount = local_amount
        .amount()
        .checked_sub(token_fee)
        .unwrap_or_default()
        .checked_sub(relayer_fee)
        .unwrap_or_default();
    if new_amount.is_zero() {
        return Err(StdError::generic_err(
            "Not enough transfer amount to cover the token and relayer fees",
        ));
    }
    Ok((new_amount, token_fee, relayer_fee))
}

pub fn deduct_token_fee(
    storage: &mut dyn Storage,
    remote_token_denom: &str,
    amount: Uint128,
    local_token_denom: &str,
) -> StdResult<(Uint128, Uint128)> {
    let token_fee = TOKEN_FEE.may_load(storage, &remote_token_denom)?;
    if let Some(token_fee) = token_fee {
        let fee = deduct_fee(token_fee, amount);
        TOKEN_FEE_ACCUMULATOR.update(
            storage,
            local_token_denom,
            |prev_fee| -> StdResult<Uint128> { Ok(prev_fee.unwrap_or_default().checked_add(fee)?) },
        )?;
        let new_deducted_amount = amount.checked_sub(fee)?;
        return Ok((new_deducted_amount, fee));
    }
    Ok((amount, Uint128::from(0u64)))
}

pub fn deduct_relayer_fee(
    storage: &mut dyn Storage,
    api: &dyn Api,
    remote_address: &str,
    remote_token_denom: &str,
    amount: Uint128,         // local amount
    offer_amount: Uint128,   // offer amount of token that swaps to ORAI
    local_token_denom: &str, // local denom
    token_price: Uint128,
) -> StdResult<(Uint128, Uint128)> {
    // api.debug(format!("token price: {}", token_price).as_str());
    if token_price.is_zero() {
        return Ok((amount, Uint128::from(0u64)));
    }

    // this is bech32 prefix of sender from other chains. Should not error because we are in the cosmos ecosystem. Every address should have prefix
    // evm case, need to filter remote token denom since prefix is always oraib
    let mut prefix = get_prefix_decode_bech32(remote_address)?;
    // api.debug(format!("prefix: {}", prefix).as_str());
    if prefix.eq(ORAIBRIDGE_PREFIX) {
        prefix = convert_remote_denom_to_evm_prefix(remote_token_denom);
    }
    // api.debug(format!("prefix after evm prefix: {}", prefix).as_str());
    let relayer_fee = RELAYER_FEE.may_load(storage, &prefix)?;
    // api.debug(format!("relayer fee: {}", relayer_fee.unwrap_or_default()).as_str());
    // no need to deduct fee if no fee is found in the mapping
    if relayer_fee.is_none() {
        return Ok((amount, Uint128::from(0u64)));
    }
    let relayer_fee = relayer_fee.unwrap();
    let required_fee_needed = relayer_fee
        .checked_mul(offer_amount)
        .unwrap_or_default()
        .checked_div(token_price)
        .unwrap_or_default();
    // api.debug(format!("required fee needed: {}", required_fee_needed).as_str());
    // accumulate fee so that we can collect it later after everything
    // we share the same accumulator because it's the same data structure, and we are accumulating so it's fine
    RELAYER_FEE_ACCUMULATOR.update(
        storage,
        local_token_denom,
        |prev_fee| -> StdResult<Uint128> {
            Ok(prev_fee
                .unwrap_or_default()
                .checked_add(required_fee_needed)?)
        },
    )?;
    Ok((
        amount.checked_sub(required_fee_needed).unwrap_or_default(),
        required_fee_needed,
    ))
}

pub fn deduct_fee(token_fee: Ratio, amount: Uint128) -> Uint128 {
    // ignore case where denominator is zero since we cannot divide with 0
    if token_fee.denominator == 0 {
        return Uint128::from(0u64);
    }
    amount.mul(Decimal::from_ratio(
        token_fee.numerator,
        token_fee.denominator,
    ))
}

pub fn convert_remote_denom_to_evm_prefix(remote_denom: &str) -> String {
    match remote_denom.split_once("0x") {
        Some((evm_prefix, _)) => return evm_prefix.to_string(),
        None => "".to_string(),
    }
}

pub fn collect_fee_msgs(
    storage: &mut dyn Storage,
    receiver: String,
    fee_accumulator: Map<&str, Uint128>,
) -> StdResult<Vec<CosmosMsg>> {
    let cosmos_msgs = fee_accumulator
        .range(storage, None, None, Order::Ascending)
        .filter_map(|data| {
            data.map(|fee_info| {
                if fee_info.1.is_zero() {
                    return None;
                }
                Some(Amount::from_parts(fee_info.0, fee_info.1).send_amount(receiver.clone(), None))
            })
            .ok()
        })
        .flatten()
        .collect::<Vec<_>>();
    // we reset all the accumulator keys to zero so that it wont accumulate more in the next txs. This action will be reverted if the fee payment txs fail.
    fee_accumulator
        .keys(storage, None, None, Order::Ascending)
        .collect::<Result<Vec<String>, StdError>>()?
        .into_iter()
        .for_each(|key| fee_accumulator.remove(storage, &key));
    Ok(cosmos_msgs)
}

#[entry_point]
/// check if success or failure and update balance, or return funds
pub fn ibc_packet_ack(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // Design decision: should we trap error like in receive?
    // retried again and again. is that good?
    let ics20msg: Ics20Ack = from_binary(&msg.acknowledgement.data)?;
    match ics20msg {
        Ics20Ack::Result(_) => on_packet_success(deps, msg.original_packet),
        Ics20Ack::Error(err) => on_packet_failure(deps, msg.original_packet, err),
    }
}

#[entry_point]
/// return fund to original sender (same as failure in ibc_packet_ack)
pub fn ibc_packet_timeout(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    let packet = msg.packet;
    on_packet_failure(deps, packet, "timeout".to_string())
}

// update the balance stored on this (channel, denom) index
fn on_packet_success(_deps: DepsMut, packet: IbcPacket) -> Result<IbcBasicResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;

    // similar event messages like ibctransfer module
    let attributes = vec![
        attr("action", "acknowledge"),
        attr("sender", &msg.sender),
        attr("receiver", &msg.receiver),
        attr("denom", &msg.denom),
        attr("amount", msg.amount),
        attr("success", "true"),
    ];

    // if let Some(memo) = msg.memo {
    //     attributes.push(attr("memo", memo));
    // }

    Ok(IbcBasicResponse::new().add_attributes(attributes))
}

// return the tokens to sender. The only refund function we have. This is to replicate the refund mechanism of ibc transfer when receiving ack fail from the remote chain
fn on_packet_failure(
    deps: DepsMut,
    packet: IbcPacket,
    err: String,
) -> Result<IbcBasicResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;

    // in case that the denom is not in the mapping list, meaning that it is not transferred back, but transfer originally from this local chain
    if ics20_denoms().may_load(deps.storage, &msg.denom)?.is_none() {
        return Ok(IbcBasicResponse::new());
    }

    let sub_msg = handle_on_packet_failure(
        deps.storage,
        &msg.sender,
        &msg.denom,
        msg.amount,
        &packet.src.channel_id,
    )?;

    let res = IbcBasicResponse::new()
        .add_submessage(sub_msg)
        .add_attribute("action", "acknowledge")
        .add_attribute("sender", msg.sender)
        .add_attribute("receiver", msg.receiver)
        .add_attribute("denom", msg.denom)
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("success", "false")
        .add_attribute("error", err);

    Ok(res)

    // send ack fail to custom contract for refund
}

pub fn handle_on_packet_failure(
    storage: &mut dyn Storage,
    packet_sender: &str,
    packet_denom: &str,
    packet_amount: Uint128,
    src_channel_id: &str,
) -> Result<SubMsg, ContractError> {
    // get ibc denom mapping to get cw20 denom & from decimals in case of packet failure, we can refund the corresponding user & amount
    let pair_mapping = ics20_denoms().load(storage, &packet_denom)?;
    let to_send = Amount::from_parts(
        parse_asset_info_denom(pair_mapping.asset_info),
        convert_remote_to_local(
            packet_amount,
            pair_mapping.remote_decimals,
            pair_mapping.asset_info_decimals,
        )?,
    );
    let cosmos_msg = to_send.send_amount(packet_sender.to_string(), None);

    // since we reduce the channel's balance optimistically when transferring back, we undo reduce it again when receiving failed ack
    undo_reduce_channel_balance(
        storage,
        src_channel_id,
        packet_denom,
        packet_amount.clone(),
        false,
    )?;

    // used submsg here & reply on error. This means that if the refund process fails => tokens will be locked in this IBC Wasm contract. We will manually handle that case. No retry
    // similar event messages like ibctransfer module
    Ok(SubMsg::reply_on_error(cosmos_msg, ACK_FAILURE_ID))
}

pub fn build_ibc_send_packet(
    amount: Uint128,
    denom: &str,
    sender: &str,
    receiver: &str,
    memo: Option<String>,
    src_channel: &str,
    timeout: IbcTimeout,
) -> StdResult<IbcMsg> {
    // build ics20 packet
    let packet = Ics20Packet::new(
        amount.clone(),
        denom, // we use ibc denom in form <transfer>/<channel>/<denom> so that when it is sent back to remote chain, it gets parsed correctly and burned
        sender,
        receiver,
        memo,
    );
    packet
        .validate()
        .map_err(|err| StdError::generic_err(err.to_string()))?;

    // prepare ibc message
    Ok(IbcMsg::SendPacket {
        channel_id: src_channel.to_string(),
        data: to_binary(&packet)?,
        timeout: timeout.into(),
    })
}
