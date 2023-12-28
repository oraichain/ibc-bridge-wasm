use std::ops::Mul;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, coin, entry_point, from_binary, to_binary, Addr, Api, Binary, CosmosMsg, Decimal, Deps,
    DepsMut, Env, Ibc3ChannelOpenResponse, IbcBasicResponse, IbcChannel, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcEndpoint, IbcMsg, IbcOrder, IbcPacket,
    IbcPacketAckMsg, IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, IbcTimeout,
    Order, QuerierWrapper, Reply, Response, StdError, StdResult, Storage, SubMsg, SubMsgResult,
    Timestamp, Uint128,
};
use cw20_ics20_msg::converter::ConvertType;
use cw20_ics20_msg::helper::{
    denom_to_asset_info, get_prefix_decode_bech32, parse_asset_info_denom,
};
use cw20_ics20_msg::receiver::DestinationInfo;
use cw_storage_plus::Map;
use oraiswap::asset::{Asset, AssetInfo};
use oraiswap::router::{RouterController, SwapOperation};

use crate::error::{ContractError, Never};
use crate::msg::{ExecuteMsg, FeeData, FollowUpMsgsData, PairQuery};
use crate::query_helper::get_destination_info_on_orai;
use crate::state::{
    get_key_ics20_ibc_denom, ics20_denoms, undo_reduce_channel_balance, ChannelInfo,
    ConvertReplyArgs, Ratio, ALLOW_LIST, CHANNEL_INFO, CONFIG, CONVERT_REPLY_ARGS, RELAYER_FEE,
    REPLY_ARGS, SINGLE_STEP_REPLY_ARGS, TOKEN_FEE,
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

// pub const RECEIVE_ID: u64 = 1337;
pub const NATIVE_RECEIVE_ID: u64 = 1338;
pub const FOLLOW_UP_IBC_SEND_FAILURE_ID: u64 = 1339;
pub const REFUND_FAILURE_ID: u64 = 1340;
pub const IBC_TRANSFER_NATIVE_ERROR_ID: u64 = 1341;
pub const SWAP_OPS_FAILURE_ID: u64 = 1342;
pub const CONVERT_FAILURE_ID: u64 = 1343;
pub const ACK_FAILURE_ID: u64 = 64023;

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        // happens only when send cw20 amount to recipient failed. Wont refund because this case is unlikely to happen
        NATIVE_RECEIVE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            // we all set ack success so that the token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
            // so no undo increase
            SubMsgResult::Err(err) => Ok(Response::new()
                .set_data(ack_success())
                .add_attribute("action", "native_receive_id")
                .add_attribute("error_transferring_ibc_tokens_to_cw20", err)),
        },
        // happens when swap failed. Will refund by sending to the initial receiver of the packet receive, amount is local on Oraichain & send through cw20
        SWAP_OPS_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            // we all set ack success so that the token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
            // so no undo increase
            SubMsgResult::Err(err) => {
                let reply_args = REPLY_ARGS.load(deps.storage)?;
                REPLY_ARGS.remove(deps.storage);
                let sub_msg = handle_packet_refund(
                    deps.storage,
                    &reply_args.local_receiver,
                    &reply_args.denom,
                    reply_args.amount,
                )?;

                Ok(Response::new()
                    .set_data(ack_success())
                    .add_submessage(sub_msg)
                    .add_attribute("action", "swap_ops_failure_id")
                    .add_attribute("error_swap_ops", err))
            }
        },
        // happens when failed to ibc send the packet to another chain after receiving the packet from the first remote chain.
        // also when swap is successful. Will refund similarly to swap ops
        FOLLOW_UP_IBC_SEND_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => {
                let reply_args = SINGLE_STEP_REPLY_ARGS.load(deps.storage)?;
                SINGLE_STEP_REPLY_ARGS.remove(deps.storage);
                // only time where we undo reduce chann balance because this message is sent and reduced optimistically on Oraichain. If fail then we undo and then refund
                undo_reduce_channel_balance(
                    deps.storage,
                    &reply_args.channel,
                    &reply_args.denom,
                    reply_args.amount,
                )?;

                let sub_msg = handle_packet_refund(
                    deps.storage,
                    &reply_args.local_receiver,
                    &reply_args.denom,
                    reply_args.amount,
                )?;
                Ok(Response::new()
                    // we all set ack success so that this token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
                    .set_data(ack_success())
                    .add_submessage(sub_msg)
                    .add_attributes(vec![
                        attr("action", "follow_up_failure_id"),
                        attr("error_ibc_send_failure", err),
                        attr("undo_reduce_channel", reply_args.channel),
                        attr("undo_reduce_channel_ibc_denom", reply_args.denom),
                        attr("undo_reduce_channel_balance", reply_args.amount),
                        attr("refund_recipient", reply_args.local_receiver),
                    ]))
            }
        },
        // fallback case when refund fails. Wont retry => will refund manually
        REFUND_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => Ok(Response::new()
                // we all set ack success so that this token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
                .set_data(ack_success())
                .add_attribute("action", "refund_failure_id")
                .add_attribute("error_trying_to_refund_single_step", err)),
        },
        // fallback case when we dont have a mapping and have to do IBC transfer and it also failed. Wont refund because it is a rare case as we dont use IBC transfer as much
        // this means that we are sending to a normal ibc transfer channel, not ibc wasm.
        IBC_TRANSFER_NATIVE_ERROR_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => Ok(Response::new()
                // we all set ack success so that this token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
                .set_data(ack_success())
                .add_attribute("action", "ibc_transfer_native_error_id")
                .add_attribute("error_trying_to_transfer_ibc_native_with_error", err)),
        },
        // happens when convert failed. Will refund by sending to the initial receiver of the packet receive, amount is local on Oraichain & send through cw20
        CONVERT_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            // we all set ack success so that the token is stuck on Oraichain, not on OraiBridge because if ack fail => token refunded on OraiBridge yet still refund on Oraichain
            // so no undo increase
            SubMsgResult::Err(err) => {
                let reply_args = CONVERT_REPLY_ARGS.load(deps.storage)?;
                CONVERT_REPLY_ARGS.remove(deps.storage);
                let sub_msg = handle_asset_refund(reply_args.local_receiver, reply_args.asset)?;

                Ok(Response::new()
                    .set_data(ack_success())
                    .add_submessage(sub_msg)
                    .add_attribute("action", "convert_failure_id")
                    .add_attribute("error_convert_ops", err))
            }
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
) -> Result<Option<Ibc3ChannelOpenResponse>, ContractError> {
    enforce_order_and_version(msg.channel(), msg.counterparty_version())?;
    Ok(None)
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

    do_ibc_packet_receive(
        deps.storage,
        deps.api,
        &deps.querier,
        env,
        &packet,
        &msg.relayer.into_string(),
    )
    .or_else(|err| {
        Ok(IbcReceiveResponse::new()
            // trade-off between reentrancy & refunding. If error, then it should be a serious error => refund to oraibridge
            // that's better than trying to update balance & let it stay in this contract and expose to reentrancy
            .set_ack(ack_fail(err.to_string()))
            .add_attributes(vec![
                attr("action", "receive"),
                attr("success", "false"),
                attr("error", err.to_string()),
                attr("src_channel_id", packet.src.channel_id),
                attr("dst_channel_id", packet.dest.channel_id),
                attr("packet_data", packet.data.to_base64()),
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

pub fn parse_ibc_info_without_sanity_checks<'a>(
    ibc_denom: &'a str,
) -> StdResult<(&'a str, &'a str, &'a str)> {
    let split_denom: Vec<&str> = ibc_denom.splitn(3, '/').collect();

    if split_denom.len() != 3 {
        return Err(StdError::generic_err(
            ContractError::NoForeignTokens {}.to_string(),
        ));
    }
    Ok((split_denom[0], split_denom[1], split_denom[2]))
}

// this does the work of ibc_packet_receive, we wrap it to turn errors into acknowledgements
fn do_ibc_packet_receive(
    storage: &mut dyn Storage,
    api: &dyn Api,
    querier: &QuerierWrapper,
    env: Env,
    packet: &IbcPacket,
    relayer: &str,
) -> Result<IbcReceiveResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;
    // let channel = packet.dest.channel_id.clone();

    // If the token originated on the remote chain, it looks like "ucosm".
    // If it originated on our chain, it looks like "port/channel/ucosm".
    let denom = parse_voucher_denom(&msg.denom, &packet.src)?;

    // if denom is native, we handle it the native way
    if denom.1 {
        return handle_ibc_packet_receive_native_remote_chain(
            storage, api, &querier, env, &denom.0, &packet, &msg, relayer,
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
    relayer: &str,
) -> Result<IbcReceiveResponse, ContractError> {
    let config = CONFIG.load(storage)?;
    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
    let ibc_packet_amount = msg.amount.to_string();
    let attributes = vec![
        ("action", "receive_native"),
        ("sender", &msg.sender),
        ("receiver", &msg.receiver),
        ("denom", denom),
        ("amount", &ibc_packet_amount),
        ("success", "true"),
        ("relayer", relayer),
    ];

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

    let mut fee_data = process_deduct_fee(
        storage,
        querier,
        api,
        &msg.sender,
        &msg.denom,
        to_send.clone(),
        pair_mapping.asset_info_decimals,
        &config.swap_router_contract,
    )?;

    // let destination = DestinationInfo::from_str(&msg.memo.clone().unwrap_or_default());
    let destination =
        DestinationInfo::from_binary(&Binary::from_base64(&msg.memo.clone().unwrap_or_default())?)
            .map_err(|err| ContractError::InvalidDestinationMemo {
                error: err.to_string(),
            })?;

    let (destination_asset_info_on_orai, destination_pair_mapping) = get_destination_info_on_orai(
        storage,
        api,
        &env,
        destination.destination_channel.clone(),
        destination.destination_denom.clone(),
    );

    // if there's a round trip in the destination then we charge additional token and relayer fees
    if !destination.destination_denom.is_empty() && !destination.destination_channel.is_empty() {
        // if there's a round trip to a different network, we deduct the token fee based on the remote destination denom
        // for relayer fee, we need to deduct using the destination network
        let (_, additional_token_fee) =
            deduct_token_fee(storage, &destination.destination_denom, to_send.amount())?;
        fee_data.token_fee = Amount::from_parts(
            fee_data.token_fee.denom(),
            fee_data
                .token_fee
                .amount()
                .checked_add(additional_token_fee)?,
        );
        let additional_relayer_fee = deduct_relayer_fee(
            storage,
            api,
            &destination.receiver,
            &destination.destination_denom,
            fee_data.token_simulate_amount,
            fee_data.token_exchange_rate_with_orai,
        )?;

        fee_data.relayer_fee = Amount::from_parts(
            fee_data.relayer_fee.denom(),
            fee_data
                .relayer_fee
                .amount()
                .checked_add(additional_relayer_fee)?,
        );

        fee_data.deducted_amount = fee_data
            .deducted_amount
            .checked_sub(additional_token_fee.checked_add(additional_relayer_fee)?)
            .unwrap_or_default();
    }

    // if the fees have consumed all user funds, we send all the fees to our token fee receiver
    if fee_data.deducted_amount.is_zero() {
        return Ok(IbcReceiveResponse::new()
            .set_ack(ack_success())
            .add_message(to_send.send_amount(config.token_fee_receiver.into_string(), None))
            .add_attributes(attributes)
            .add_attributes(vec![
                ("token_fee", &fee_data.token_fee.amount().to_string()),
                ("relayer_fee", &fee_data.relayer_fee.amount().to_string()),
            ]));
    }
    if !fee_data.token_fee.is_empty() {
        cosmos_msgs.push(
            fee_data
                .token_fee
                .send_amount(config.token_fee_receiver.into_string(), None),
        )
    }
    if !fee_data.relayer_fee.is_empty() {
        cosmos_msgs.push(fee_data.relayer_fee.send_amount(relayer.to_string(), None))
    }

    let new_deducted_to_send = Amount::from_parts(to_send.denom(), fee_data.deducted_amount);
    let follow_up_msg_data = get_follow_up_msgs(
        storage,
        api,
        querier,
        env.clone(),
        new_deducted_to_send,
        pair_mapping.asset_info,
        destination_asset_info_on_orai,
        &msg.sender,
        &msg.receiver,
        &destination,
        packet.dest.channel_id.as_str(),
        destination_pair_mapping,
    )?;

    // increase channel balance submsg. We increase it first before doing other tasks
    cosmos_msgs.push(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::IncreaseChannelBalanceIbcReceive {
            dest_channel_id: packet.dest.channel_id.clone(),
            ibc_denom,
            amount: msg.amount,
            local_receiver: msg.receiver.clone(),
        })?,
        funds: vec![],
    }));
    let mut res = IbcReceiveResponse::new()
        .set_ack(ack_success())
        .add_messages(cosmos_msgs)
        .add_submessages(follow_up_msg_data.sub_msgs)
        .add_attributes(attributes)
        .add_attributes(vec![
            ("token_fee", &fee_data.token_fee.amount().to_string()),
            ("relayer_fee", &fee_data.relayer_fee.amount().to_string()),
        ]);
    if !follow_up_msg_data.follow_up_msg.is_empty() {
        res = res.add_attribute("ibc_error_msg", follow_up_msg_data.follow_up_msg);
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
    destination_asset_info_on_orai: AssetInfo,
    ibc_sender: &str,    // will be receiver  of ics20 packet if destination is evm
    orai_receiver: &str, // receiver on Oraichain
    destination: &DestinationInfo,
    initial_dest_channel_id: &str, // channel id on Oraichain receiving the token from other chain,
    destination_pair_mapping: Option<PairQuery>,
) -> Result<FollowUpMsgsData, ContractError> {
    let config = CONFIG.load(storage)?;
    let mut sub_msgs: Vec<SubMsg> = vec![];
    let send_only_sub_msg = SubMsg::reply_on_error(
        to_send.send_amount(orai_receiver.to_string(), None),
        NATIVE_RECEIVE_ID,
    );
    let mut follow_up_msgs_data = FollowUpMsgsData {
        sub_msgs: vec![send_only_sub_msg],
        follow_up_msg: "".to_string(),
        is_success: true,
    };

    if destination.destination_denom.is_empty() {
        return Ok(follow_up_msgs_data);
    }

    // successful case. We dont care if this msg is going to be successful or not because it does not affect our ibc receive flow (just submsgs)
    let mut target_asset_info_on_swap = destination_asset_info_on_orai.clone();

    // check destination_asset_info_on_orai should convert before ibc transfer
    let converter_info = config
        .converter_contract
        .converter_info(querier, &destination_asset_info_on_orai);

    if let Some(converter_info) = converter_info.clone() {
        target_asset_info_on_swap = converter_info.token_ratio.info;
    }

    let swap_operations = build_swap_operations(
        target_asset_info_on_swap.clone(),
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
            follow_up_msgs_data.follow_up_msg = format!(
                "Cannot simulate swap with ops: {:?} with error: {:?}",
                swap_operations,
                response.unwrap_err().to_string()
            );
            follow_up_msgs_data.is_success = false;

            return Ok(follow_up_msgs_data);
        }
        minimum_receive = response.unwrap().amount;
    }

    // by default, the receiver is the original address sent in ics20packet
    let mut to = Some(api.addr_validate(orai_receiver)?);

    // build convert msg
    let mut send_converted_msg: Option<CosmosMsg> = None;
    if converter_info.is_some() {
        let from_asset = Asset {
            amount: minimum_receive,
            info: target_asset_info_on_swap,
        };

        let (convert_msg, return_asset) = config.converter_contract.process_convert(
            querier,
            &destination_asset_info_on_orai,
            minimum_receive,
            ConvertType::ToSource,
        )?;

        if let Some(convert_msg) = convert_msg {
            CONVERT_REPLY_ARGS.save(
                storage,
                &ConvertReplyArgs {
                    local_receiver: orai_receiver.to_string(),
                    asset: from_asset,
                },
            )?;
            sub_msgs.push(SubMsg::reply_on_error(convert_msg, CONVERT_FAILURE_ID));
            minimum_receive = return_asset.amount;

            to = None;
            send_converted_msg = Some(
                Amount::from_parts(parse_asset_info_denom(return_asset.info), minimum_receive)
                    .send_amount(orai_receiver.to_string(), None),
            );
        }
    }

    let mut build_ibc_msg_result = build_ibc_msg(
        env,
        orai_receiver,
        initial_dest_channel_id,
        minimum_receive.clone(),
        &ibc_sender,
        &destination,
        config.default_timeout,
        destination_pair_mapping,
        destination_asset_info_on_orai,
    );

    if let Some(ibc_msg) = build_ibc_msg_result.as_mut().ok() {
        sub_msgs.append(ibc_msg);
        // if there's an ibc msg => swap receiver is None so the receiver is this ibc wasm address
        to = None;
    } else {
        follow_up_msgs_data.follow_up_msg = build_ibc_msg_result.unwrap_err().to_string();
        // if has converter message, but don't have ibc messages, must send return amount after convert to receiver
        if let Some(send_converted_msg) = send_converted_msg {
            sub_msgs.push(SubMsg::reply_on_error(
                send_converted_msg,
                NATIVE_RECEIVE_ID,
            ));
        }

        // if destination_channel is empty, still success
        if !destination.destination_channel.is_empty() {
            follow_up_msgs_data.is_success = false;
        }
    };

    build_swap_msgs(
        minimum_receive,
        &config.swap_router_contract,
        to_send.amount(),
        initial_receive_asset_info,
        to.clone(),
        &mut sub_msgs,
        swap_operations,
    )?;
    // fallback case. If there's no cosmos message then we return send amount
    if sub_msgs.is_empty() {
        return Ok(follow_up_msgs_data);
    };
    follow_up_msgs_data.sub_msgs = sub_msgs;
    return Ok(follow_up_msgs_data);
}

pub fn build_swap_operations(
    destination_asset_info_on_orai: AssetInfo,
    initial_receive_asset_info: AssetInfo,
    fee_denom: &str,
) -> Vec<SwapOperation> {
    // always swap with orai first cuz its base denom & every token has a pair with it
    let fee_denom_asset_info = AssetInfo::NativeToken {
        denom: fee_denom.to_string(),
    };
    let mut swap_operations = vec![];
    if destination_asset_info_on_orai.eq(&initial_receive_asset_info) {
        return vec![];
    }
    if initial_receive_asset_info.ne(&fee_denom_asset_info) {
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: initial_receive_asset_info.clone(),
            ask_asset_info: fee_denom_asset_info.clone(),
        })
    }
    if destination_asset_info_on_orai.to_string().ne(fee_denom) {
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: fee_denom_asset_info.clone(),
            ask_asset_info: destination_asset_info_on_orai,
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
    sub_msgs: &mut Vec<SubMsg>,
    operations: Vec<SwapOperation>,
) -> StdResult<()> {
    // the swap msg must be executed before other msgs because we need the ask token amount to create ibc msg => insert in first index
    if operations.len() == 0 {
        return Ok(());
    }
    // double check. We cannot let swap ops with Some(to) aka swap to someone else, not this contract and then transfer ibc => would be double spending
    if to.is_some() && sub_msgs.len() > 0 {
        // forbidden case. Pop all sub messages and return empty
        while sub_msgs.pop().is_some() {
            sub_msgs.pop();
        }
        return Ok(());
    }
    sub_msgs.insert(
        0,
        SubMsg::reply_on_error(
            swap_router_contract.execute_operations(
                initial_receive_asset_info,
                amount,
                operations,
                Some(minimum_receive),
                to,
            )?,
            SWAP_OPS_FAILURE_ID,
        ),
    );

    Ok(())
}

pub fn build_ibc_msg(
    env: Env,
    local_receiver: &str,
    local_channel_id: &str,
    amount: Uint128,
    remote_address: &str,
    destination: &DestinationInfo,
    default_timeout: u64,
    pair_mapping: Option<PairQuery>,
    destination_asset_info_on_orai: AssetInfo,
) -> StdResult<Vec<SubMsg>> {
    // if there's no dest channel then we stop, no need to transfer ibc
    if destination.destination_channel.is_empty() {
        return Err(StdError::generic_err(
            "Destination channel empty in build ibc msg",
        ));
    }
    let timeout = env.block.time.plus_seconds(default_timeout);
    let (is_evm_based, _) = destination.is_receiver_evm_based();
    if is_evm_based {
        if let Some(mapping) = pair_mapping {
            return Ok(process_ibc_msg(
                mapping,
                env.contract.address.to_string(),
                local_receiver,
                local_channel_id,
                env.contract.address.as_str(),
                remote_address, // use sender from ICS20Packet as receiver when transferring back because we have the actual receiver in memo for evm cases
                Some(destination.receiver.clone()),
                amount,
                timeout,
            )?);
        }
        return Err(StdError::generic_err("cannot find pair mappings"));
    }
    // 2nd case, where destination network is not evm, but it is still supported on our channel (eg: cw20 ATOM mapped with native ATOM on Cosmos), then we call
    let is_cosmos_based = destination.is_receiver_cosmos_based();
    if is_cosmos_based {
        if let Some(mapping) = pair_mapping {
            return Ok(process_ibc_msg(
                mapping,
                env.contract.address.to_string(),
                local_receiver,
                &destination.destination_channel,
                env.contract.address.as_str(),
                &destination.receiver, // now we use dest receiver since cosmos based universal swap wont be sent to oraibridge, so the receiver is the correct receive addr
                None, // no need memo because it is not used in the remote cosmos based chain
                amount,
                timeout,
            )?);
        }

        // final case, where the destination token is from a remote chain that we dont have a pair mapping with.
        // we use ibc transfer so that attackers cannot manipulate the data to send to oraibridge without reducing the channel balance
        // by using ibc transfer, the contract must actually owns native ibc tokens, which is not possible if it's oraibridge tokens
        // we do not need to reduce channel balance because this transfer is not on our contract channel, but on destination channel
        let ibc_msg: CosmosMsg = match destination_asset_info_on_orai {
            AssetInfo::NativeToken { denom } => IbcMsg::Transfer {
                channel_id: destination.destination_channel.clone(),
                to_address: destination.receiver.clone(),
                amount: coin(amount.u128(), denom),
                timeout: IbcTimeout::with_timestamp(timeout),
            }
            .into(),
            AssetInfo::Token { contract_addr: _ } => {
                return Err(StdError::generic_err("The destination must be denom"))
            }
        };
        // let ibc_msg: CosmosMsg = IbcMsg::Transfer {
        //     channel_id: destination.destination_channel.clone(),
        //     to_address: destination.receiver.clone(),
        //     amount: coin(amount.u128(), destination.destination_denom.clone()),
        //     timeout: IbcTimeout::with_timestamp(timeout),
        // }
        // .into();
        return Ok(vec![SubMsg::reply_on_error(
            ibc_msg,
            IBC_TRANSFER_NATIVE_ERROR_ID,
        )]);
    }
    Err(StdError::generic_err(
        "The destination info is neither evm or cosmos based",
    ))
}

// TODO: Write unit tests for relayer fee & cosmos based universal swap in simulate js
pub fn process_ibc_msg(
    pair_query: PairQuery,
    contract_addr: String,
    local_receiver: &str,
    src_channel: &str,
    ibc_msg_sender: &str,
    ibc_msg_receiver: &str,
    memo: Option<String>,
    amount: Uint128,
    timeout: Timestamp,
) -> StdResult<Vec<SubMsg>> {
    let remote_amount = convert_local_to_remote(
        amount,
        pair_query.pair_mapping.remote_decimals,
        pair_query.pair_mapping.asset_info_decimals,
    )?;

    // prepare ibc message
    let msg: CosmosMsg = build_ibc_send_packet(
        remote_amount,
        &pair_query.key,
        ibc_msg_sender,
        ibc_msg_receiver,
        memo,
        src_channel,
        timeout.into(),
    )?
    .into();

    let reduce_balance_msg = SubMsg::new(CosmosMsg::Wasm(cosmwasm_std::WasmMsg::Execute {
        contract_addr,
        msg: to_binary(&ExecuteMsg::ReduceChannelBalanceIbcReceive {
            src_channel_id: src_channel.to_string(),
            ibc_denom: pair_query.key.clone(),
            amount: remote_amount.clone(),
            local_receiver: local_receiver.to_string(),
        })?,
        funds: vec![],
    }));

    Ok(vec![
        reduce_balance_msg,
        SubMsg::reply_on_error(msg, FOLLOW_UP_IBC_SEND_FAILURE_ID),
    ])
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
) -> StdResult<FeeData> {
    let local_denom = local_amount.denom();
    let (deducted_amount, token_fee) =
        deduct_token_fee(storage, remote_token_denom, local_amount.amount())?;
    // simulate for relayer fee
    let offer_asset_info = denom_to_asset_info(api, &local_amount.raw_denom());
    let simulate_amount = Uint128::from(10u64.pow((decimals + 1) as u32) as u64); // +1 to make sure the offer amount is large enough to swap successfully
    let exchange_rate_with_orai = get_token_price(
        querier,
        simulate_amount,
        swap_router_contract,
        offer_asset_info,
    );
    let relayer_fee = deduct_relayer_fee(
        storage,
        api,
        remote_sender,
        remote_token_denom,
        simulate_amount,
        exchange_rate_with_orai,
    )?;

    let mut fee_data = FeeData {
        deducted_amount: deducted_amount.checked_sub(relayer_fee).unwrap_or_default(),
        token_fee: Amount::from_parts(local_denom.clone(), token_fee),
        relayer_fee: Amount::from_parts(local_denom.clone(), relayer_fee),
        token_simulate_amount: simulate_amount,
        token_exchange_rate_with_orai: exchange_rate_with_orai,
    };

    // if after token fee, the deducted amount is 0 then we deduct all to token fee
    if deducted_amount.is_zero() {
        fee_data.deducted_amount = Uint128::zero();
        fee_data.token_fee = local_amount;
        fee_data.relayer_fee = Amount::from_parts(local_denom, Uint128::zero());
        return Ok(fee_data);
    }
    // if the relayer fee makes the final amount 0, then we charge the remaining deducted amount as relayer fee
    if fee_data.deducted_amount.is_zero() {
        fee_data.deducted_amount = Uint128::zero();
        fee_data.token_fee = Amount::from_parts(local_denom.clone(), token_fee);
        fee_data.relayer_fee = Amount::from_parts(local_denom.clone(), deducted_amount);
        return Ok(fee_data);
    }
    Ok(fee_data)
}

pub fn deduct_token_fee(
    storage: &mut dyn Storage,
    remote_token_denom: &str,
    amount: Uint128,
) -> StdResult<(Uint128, Uint128)> {
    let token_fee = TOKEN_FEE.may_load(storage, &remote_token_denom)?;
    if let Some(token_fee) = token_fee {
        let fee = deduct_fee(token_fee, amount);
        let new_deducted_amount = amount.checked_sub(fee)?;
        return Ok((new_deducted_amount, fee));
    }
    Ok((amount, Uint128::from(0u64)))
}

pub fn deduct_relayer_fee(
    storage: &mut dyn Storage,
    _api: &dyn Api,
    remote_address: &str,
    remote_token_denom: &str,
    simulate_amount: Uint128, // offer amount of token that swaps to ORAI
    token_price: Uint128,
) -> StdResult<Uint128> {
    // api.debug(format!("token price: {}", token_price).as_str());
    if token_price.is_zero() {
        return Ok(Uint128::from(0u64));
    }

    // this is bech32 prefix of sender from other chains. Should not error because we are in the cosmos ecosystem. Every address should have prefix
    // evm case, need to filter remote token denom since prefix is always oraib
    let prefix_result = get_prefix_decode_bech32(remote_address);
    // api.debug(format!("prefix: {}", prefix).as_str());
    let prefix: String = if prefix_result.is_err() {
        convert_remote_denom_to_evm_prefix(remote_token_denom)
    } else {
        let prefix = prefix_result.unwrap();
        if prefix.eq(ORAIBRIDGE_PREFIX) {
            convert_remote_denom_to_evm_prefix(remote_token_denom)
        } else {
            prefix
        }
    };
    // api.debug(format!("prefix after evm prefix: {}", prefix).as_str());
    let relayer_fee = RELAYER_FEE.may_load(storage, &prefix)?;
    // api.debug(format!("relayer fee: {}", relayer_fee.unwrap_or_default()).as_str());
    // no need to deduct fee if no fee is found in the mapping
    if relayer_fee.is_none() {
        return Ok(Uint128::from(0u64));
    }
    let relayer_fee = relayer_fee.unwrap();
    let required_fee_needed = relayer_fee
        .checked_mul(simulate_amount)
        .unwrap_or_default()
        .checked_div(token_price)
        .unwrap_or_default();
    Ok(required_fee_needed)
}

pub fn deduct_fee(token_fee: Ratio, amount: Uint128) -> Uint128 {
    // ignore case where denominator is zero since we cannot divide with 0
    if token_fee.denominator == 0 {
        return Uint128::from(0u64);
    }
    amount.mul(Decimal::from_ratio(
        token_fee.nominator,
        token_fee.denominator,
    ))
}

pub fn get_token_price(
    querier: &QuerierWrapper,
    simulate_amount: Uint128,
    swap_router_contract: &RouterController,
    offer_asset_info: AssetInfo,
) -> Uint128 {
    let orai_asset_info = AssetInfo::NativeToken {
        denom: "orai".to_string(),
    };
    if offer_asset_info.eq(&orai_asset_info) {
        return simulate_amount;
    }
    let token_price = swap_router_contract
        .simulate_swap(
            querier,
            simulate_amount,
            vec![SwapOperation::OraiSwap {
                offer_asset_info,
                // always swap with orai. If it does not share a pool with ORAI => ignore, no fee
                ask_asset_info: orai_asset_info,
            }],
        )
        .map(|data| data.amount)
        .unwrap_or_default();
    token_price
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
    fee_accumulator.clear(storage);
    Ok(cosmos_msgs)
}

pub fn find_evm_pair_mapping(
    ibc_denom_pair_mapping_key: &str,
    evm_prefix: &str,
    destination_channel: &str,
) -> bool {
    // eg: 'wasm.orai195269awwnt5m6c843q6w7hp8rt0k7syfu9de4h0wz384slshuzps8y7ccm/channel-29/eth-mainnet0x4c11249814f11b9346808179Cf06e71ac328c1b5'
    // parse to get eth-mainnet0x...
    // then collect eth-mainnet prefix, and compare with dest channel
    // then we compare the dest channel with the pair mapping. They should match as well
    let (_, ibc_channel, ibc_denom) =
        parse_ibc_info_without_sanity_checks(ibc_denom_pair_mapping_key).unwrap_or_default();
    convert_remote_denom_to_evm_prefix(ibc_denom).eq(&evm_prefix)
        && ibc_channel.eq(destination_channel)
}

#[entry_point]
/// check if success or failure and update balance, or return funds
/// This entrypoint is called when we receive an acknowledgement packet from a remote chain
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

// return the tokens to sender
// only gets called when we receive an acknowledgement packet from the remote chain
// it means that the ibc packet we sent must be successful, but there's something wrong with the remote chain that they cannot receive a successful acknowledgement
// will refund because this case is different from the FOLLOW_UP_IBC_SEND_FAILURE_ID
// FOLLOW_UP_IBC_SEND_FAILURE_ID failed to send ibc packet. This one has successfully sent
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

    let sub_msg = handle_packet_refund(deps.storage, &msg.sender, &msg.denom, msg.amount)?;
    // since we reduce the channel's balance optimistically when transferring back, we undo reduce it again when receiving failed ack
    undo_reduce_channel_balance(deps.storage, &packet.src.channel_id, &msg.denom, msg.amount)?;

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

pub fn handle_packet_refund(
    storage: &mut dyn Storage,
    packet_sender: &str,
    packet_denom: &str,
    packet_amount: Uint128,
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

    // used submsg here & reply on error. This means that if the refund process fails => tokens will be locked in this IBC Wasm contract. We will manually handle that case. No retry
    // similar event messages like ibctransfer module
    Ok(SubMsg::reply_on_error(cosmos_msg, REFUND_FAILURE_ID))
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
        timeout,
    })
}

pub fn handle_asset_refund(receiver: String, asset: Asset) -> Result<SubMsg, ContractError> {
    let to_send = Amount::from_parts(parse_asset_info_denom(asset.info), asset.amount);
    let cosmos_msg = to_send.send_amount(receiver, None);

    // used submsg here & reply on error. This means that if the refund process fails => tokens will be locked in this IBC Wasm contract. We will manually handle that case. No retry
    // similar event messages like ibctransfer module
    Ok(SubMsg::reply_on_error(cosmos_msg, REFUND_FAILURE_ID))
}
