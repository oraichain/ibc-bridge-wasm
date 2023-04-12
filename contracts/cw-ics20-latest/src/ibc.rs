use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, coin, entry_point, from_binary, to_binary, Addr, Api, BankMsg, Binary, CosmosMsg, Deps,
    DepsMut, Env, IbcBasicResponse, IbcChannel, IbcChannelCloseMsg, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcEndpoint, IbcMsg, IbcOrder, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, IbcTimeout, Order,
    QuerierWrapper, Reply, Response, StdError, StdResult, Storage, SubMsg, SubMsgResult, Timestamp,
    Uint128, WasmMsg,
};
use cw20_ics20_msg::receiver::DestinationInfo;
use oraiswap::asset::AssetInfo;
use oraiswap::router::{SimulateSwapOperationsResponse, SwapOperation};

use crate::error::{ContractError, Never};
use crate::state::{
    get_key_ics20_ibc_denom, ics20_denoms, increase_channel_balance, reduce_channel_balance,
    undo_increase_channel_balance, undo_reduce_channel_balance, ChannelInfo, MappingMetadata,
    ReplyArgs, ALLOW_LIST, CHANNEL_INFO, CONFIG, REPLY_ARGS, SINGLE_STEP_REPLY_ARGS,
};
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, TokenInfoResponse};
use cw20_ics20_msg::amount::{convert_local_to_remote, convert_remote_to_local, Amount};

pub const ICS20_VERSION: &str = "ics20-1";
pub const ICS20_ORDERING: IbcOrder = IbcOrder::Unordered;

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
fn ack_fail(err: String) -> Binary {
    let res = Ics20Ack::Error(err);
    to_binary(&res).unwrap()
}

pub const RECEIVE_ID: u64 = 1337;
pub const NATIVE_RECEIVE_ID: u64 = 1338;
pub const FOLLOW_UP_MSGS_ID: u64 = 1339;
pub const ACK_FAILURE_ID: u64 = 64023;
// const TRANSFER_BACK_FAILURE_ID: u64 = 1339;

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        RECEIVE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => {
                // Important design note:  with ibcv2 and wasmd 0.22 we can implement this all much easier.
                // No reply needed... the receive function and submessage should return error on failure and all
                // state gets reverted with a proper app-level message auto-generated

                // Since we need compatibility with Juno (Jan 2022), we need to ensure that optimisitic
                // state updates in ibc_packet_receive get reverted in the (unlikely) chance of an
                // error while sending the token

                // However, this requires passing some state between the ibc_packet_receive function and
                // the reply handler. We do this with a singleton, with is "okay" for IBC as there is no
                // reentrancy on these functions (cannot be called by another contract). This pattern
                // should not be used for ExecuteMsg handlers
                let reply_args = REPLY_ARGS.load(deps.storage)?;
                undo_reduce_channel_balance(
                    deps.storage,
                    &reply_args.channel,
                    &reply_args.denom,
                    reply_args.amount,
                    true,
                )?;

                Ok(Response::new().set_data(ack_fail(err)))
            }
        },
        NATIVE_RECEIVE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => {
                // Important design note:  with ibcv2 and wasmd 0.22 we can implement this all much easier.
                // No reply needed... the receive function and submessage should return error on failure and all
                // state gets reverted with a proper app-level message auto-generated

                // Since we need compatibility with Juno (Jan 2022), we need to ensure that optimisitic
                // state updates in ibc_packet_receive get reverted in the (unlikely) chance of an
                // error while sending the token

                // However, this requires passing some state between the ibc_packet_receive function and
                // the reply handler. We do this with a singleton, with is "okay" for IBC as there is no
                // reentrancy on these functions (cannot be called by another contract). This pattern
                // should not be used for ExecuteMsg handlers
                let reply_args = REPLY_ARGS.load(deps.storage)?;
                undo_increase_channel_balance(
                    deps.storage,
                    &reply_args.channel,
                    &reply_args.denom,
                    reply_args.amount,
                    false,
                )?;

                Ok(Response::new()
                    .set_data(ack_fail(err.clone()))
                    .add_attribute("error_transferring_ibc_tokens_to_cw20", err))
            }
        },
        FOLLOW_UP_MSGS_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => {
                let reply_args = SINGLE_STEP_REPLY_ARGS.load(deps.storage).ok();
                // if there's an error but no ibc msg aka no channel balance reduce => wont undo reduce
                if let Some(reply_args) = reply_args {
                    undo_reduce_channel_balance(
                        deps.storage,
                        &reply_args.channel,
                        &reply_args.denom,
                        reply_args.amount,
                        false,
                    )?;
                }
                Ok(Response::new()
                    .set_data(ack_fail(err.clone()))
                    .add_attribute("error_follow_up_msgs", err))
            }
        },
        ACK_FAILURE_ID => match reply.result {
            SubMsgResult::Ok(_) => Ok(Response::new()),
            SubMsgResult::Err(err) => Ok(Response::new().set_data(ack_fail(err))),
        },
        // TRANSFER_BACK_FAILURE_ID => match reply.result {
        //     SubMsgResult::Ok(_) => Ok(Response::new()),
        //     SubMsgResult::Err(err) => Ok(Response::new()
        //         .set_data(ack_fail(err.clone()))
        //         .add_attribute("error_refund_cw20_tokens", err)),
        // },
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

// this does the work of ibc_packet_receive, we wrap it to turn errors into acknowledgements
fn do_ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    packet: &IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;
    let channel = packet.dest.channel_id.clone();

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

    // make sure we have enough balance for this
    reduce_channel_balance(deps.storage, &channel, denom.0, msg.amount, true)?;

    // we need to save the data to update the balances in reply
    let reply_args = ReplyArgs {
        channel,
        denom: denom.0.to_string(),
        amount: msg.amount,
    };
    REPLY_ARGS.save(deps.storage, &reply_args)?;

    let to_send = Amount::from_parts(denom.0.to_string(), msg.amount);
    let gas_limit = check_gas_limit(deps.as_ref(), &to_send)?;
    let send = send_amount(to_send, msg.receiver.clone(), None);
    let mut submsg = SubMsg::reply_on_error(send, RECEIVE_ID);
    submsg.gas_limit = gas_limit;

    let res = IbcReceiveResponse::new()
        .set_ack(ack_success())
        .add_submessage(submsg)
        .add_attribute("action", "receive")
        .add_attribute("sender", msg.sender)
        .add_attribute("receiver", msg.receiver)
        .add_attribute("denom", denom.0)
        .add_attribute("amount", msg.amount)
        .add_attribute("success", "true");

    Ok(res)
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
    // make sure we have enough balance for this

    // key in form transfer/channel-0/foo
    let ibc_denom = get_key_ics20_ibc_denom(&packet.dest.port_id, &packet.dest.channel_id, denom);
    let pair_mapping = ics20_denoms()
        .load(storage, &ibc_denom)
        .map_err(|_| ContractError::NotOnMappingList {})?;

    let to_send = Amount::from_parts(
        parse_asset_info_denom(pair_mapping.asset_info),
        convert_remote_to_local(
            msg.amount,
            pair_mapping.remote_decimals,
            pair_mapping.asset_info_decimals,
        )?,
    );
    // after receiving the cw20 amount, we try to do fee swapping for the user if needed so he / she can create txs on the network
    let submsgs: Vec<SubMsg> = get_follow_up_msgs(
        storage,
        api,
        querier,
        env.contract.address,
        to_send,
        &msg.sender,
        &msg.receiver,
        &msg.memo.clone().unwrap_or_default(),
        packet,
    )?
    .into_iter()
    .map(|msg| SubMsg::reply_on_error(msg, FOLLOW_UP_MSGS_ID))
    .collect();

    increase_channel_balance(
        storage,
        &packet.dest.channel_id,
        &ibc_denom,
        msg.amount.clone(),
        false,
    )?;

    // we need to save the data to update the balances in reply
    let reply_args = ReplyArgs {
        channel: packet.dest.channel_id.clone(),
        denom: ibc_denom.clone(),
        amount: msg.amount,
    };
    REPLY_ARGS.save(storage, &reply_args)?;

    let res = IbcReceiveResponse::new()
        .set_ack(ack_success())
        .add_submessages(submsgs)
        .add_attribute("action", "receive_native")
        .add_attribute("sender", msg.sender.clone())
        .add_attribute("receiver", msg.receiver.clone())
        .add_attribute("denom", denom)
        .add_attribute("amount", msg.amount.to_string())
        .add_attribute("success", "true");

    Ok(res)
}

// TODO: add e2e tests for this function
pub fn get_follow_up_msgs(
    storage: &mut dyn Storage,
    api: &dyn Api,
    querier: &QuerierWrapper,
    ibc_wasm_addr: Addr,
    to_send: Amount,
    sender: &str,
    receiver: &str,
    memo: &str,
    packet: &IbcPacket,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let config = CONFIG.load(storage)?;
    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
    if memo.is_empty() {
        return Ok(vec![send_amount(to_send, receiver.to_string(), None)]);
    }
    let destination: DestinationInfo = DestinationInfo::from_str(memo);
    // if destination denom, then we simply transfers cw20 to the receiver address.
    if destination.destination_denom.is_empty() {
        return Ok(vec![send_amount(
            to_send,
            destination.receiver.clone(),
            None,
        )]);
    }
    // successful case. We dont care if this msg is going to be successful or not because it does not affect our ibc receive flow (just submsgs)
    let cw20_address = api.addr_validate(&to_send.raw_denom())?;
    // if there's a receiver => swap receiver is this ibc wasm address
    let to = parse_swap_to(&destination.destination_channel, &destination.receiver);
    let receiver_asset_info = if querier
        .query_wasm_smart::<TokenInfoResponse>(
            destination.destination_denom.clone(),
            &Cw20QueryMsg::TokenInfo {},
        )
        .is_ok()
    {
        AssetInfo::Token {
            contract_addr: Addr::unchecked(destination.destination_denom.clone()),
        }
    } else {
        AssetInfo::NativeToken {
            denom: destination.destination_denom.clone(),
        }
    };
    let swap_operations = build_swap_operations(
        receiver_asset_info.clone(),
        cw20_address.clone(),
        config.fee_denom.as_str(),
        &destination,
    );

    let minimum_receive = build_swap_msgs(
        querier,
        &config.swap_router_contract,
        to_send.amount(),
        cw20_address.as_str(),
        to,
        &mut cosmos_msgs,
        swap_operations,
    )?;

    let ibc_msg = build_ibc_msg(
        storage,
        &receiver_asset_info.to_string(),
        packet.dest.channel_id.as_str(),
        minimum_receive,
        &sender,
        ibc_wasm_addr.as_str(),
        &destination,
        config.default_timeout,
    );
    if let Some(ibc_msg) = ibc_msg.ok() {
        cosmos_msgs.push(ibc_msg);
    }
    return Ok(cosmos_msgs);
}

pub fn build_swap_operations(
    receiver_asset_info: AssetInfo,
    cw20_address: Addr,
    fee_denom: &str,
    destination: &DestinationInfo,
) -> Vec<SwapOperation> {
    // always swap with orai first cuz its base denom & every token has a pair with it
    let mut swap_operations = vec![SwapOperation::OraiSwap {
        offer_asset_info: AssetInfo::Token {
            contract_addr: cw20_address.clone(),
        },
        ask_asset_info: AssetInfo::NativeToken {
            denom: fee_denom.to_string(),
        },
    }];
    // if destination denom != orai, then we add another swap op
    if !destination.destination_denom.eq(&fee_denom) {
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: AssetInfo::NativeToken {
                denom: fee_denom.to_string(),
            },
            ask_asset_info: receiver_asset_info,
        });
    }
    swap_operations
}

pub fn parse_swap_to(dest_channel: &str, dest_receiver: &str) -> Option<String> {
    // if there's a destination channel, then we wont swap to dest receiver, but swap to ibc wasm contract
    if !dest_channel.is_empty() {
        return None;
    }
    Some(dest_receiver.to_string())
}

pub fn build_swap_msgs(
    querier: &QuerierWrapper,
    swap_router_contract: &str,
    amount: Uint128,
    swap_address: &str,
    to: Option<String>,
    cosmos_msgs: &mut Vec<CosmosMsg>,
    operations: Vec<SwapOperation>,
) -> StdResult<Uint128> {
    let response: SimulateSwapOperationsResponse = querier.query_wasm_smart(
        swap_router_contract.clone(),
        &oraiswap::router::QueryMsg::SimulateSwapOperations {
            offer_amount: amount.clone(),
            operations: operations.clone(),
        },
    )?;
    cosmos_msgs.push(
        WasmMsg::Execute {
            contract_addr: swap_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: swap_router_contract.to_string(),
                amount,
                msg: to_binary(&oraiswap::router::Cw20HookMsg::ExecuteSwapOperations {
                    operations,
                    minimum_receive: Some(response.amount.clone()),
                    to,
                })?,
            })?,
            funds: vec![],
        }
        .into(),
    );
    Ok(response.amount)
}

pub fn build_ibc_msg(
    storage: &mut dyn Storage,
    receiver_asset_info: &str,
    local_channel_id: &str,
    amount: Uint128,
    remote_address: &str,
    ibc_wasm_addr: &str,
    destination: &DestinationInfo,
    timeout: u64,
) -> StdResult<CosmosMsg> {
    // if there's no dest channel then we stop, no need to transfer ibc
    if destination.destination_channel.is_empty() {
        return Err(StdError::generic_err("Destination channel empty"));
    }
    let (is_evm_based, destination) = destination.is_receiver_evm_based();
    if is_evm_based {
        // use sender from ICS20Packet as receiver when transferring back
        let pair_mappings: StdResult<Vec<(String, MappingMetadata)>> = ics20_denoms()
            .idx
            .asset_info
            .prefix(receiver_asset_info.to_string())
            .range(storage, None, None, Order::Ascending)
            .collect();

        if pair_mappings.is_err() {
            return Err(StdError::generic_err("error pair mapping"));
        }
        let pair_mappings = pair_mappings
            .unwrap()
            .into_iter()
            .find(|(key, _)| key.contains(&destination.destination_channel));
        if let Some(pair_mapping) = pair_mappings {
            let amount = convert_local_to_remote(
                amount,
                pair_mapping.1.remote_decimals,
                pair_mapping.1.asset_info_decimals,
            )?;

            // build ics20 packet
            let packet = Ics20Packet::new(
                amount.clone(),
                pair_mapping.0.clone(), // we use ibc denom in form <transfer>/<channel>/<denom> so that when it is sent back to remote chain, it gets parsed correctly and burned
                ibc_wasm_addr,
                &remote_address,
                Some(destination.receiver),
            );
            // because we are transferring back, we reduce the channel's balance
            reduce_channel_balance(
                storage,
                &local_channel_id.clone(),
                &pair_mapping.0.clone(),
                amount,
                false,
            )
            .map_err(|err| StdError::generic_err(err.to_string()))?;
            let reply_args = ReplyArgs {
                channel: local_channel_id.to_string(),
                denom: pair_mapping.0,
                amount,
            };
            // keep track of the reply. We need to keep a seperate value because if using REPLY, it could be overriden by the channel increase later on
            SINGLE_STEP_REPLY_ARGS.save(storage, &reply_args)?;

            // prepare ibc message
            let msg = IbcMsg::SendPacket {
                channel_id: local_channel_id.to_string(),
                data: to_binary(&packet)?,
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(timeout)),
            };
            return Ok(msg.into());
        }
        return Err(StdError::generic_err("cannot find pair mappings"));
    }
    // we use ibc transfer so that attackers cannot manipulate the data to send to oraibridge without reducing the channel balance
    // by using ibc transfer, the contract must actually owns native ibc tokens, which is not possible if it's oraibridge tokens
    let ibc_msg = IbcMsg::Transfer {
        channel_id: destination.destination_channel,
        to_address: destination.receiver,
        amount: coin(amount.u128(), destination.destination_denom),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(timeout)),
    };
    Ok(ibc_msg.into())
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

// return the tokens to sender
fn on_packet_failure(
    deps: DepsMut,
    packet: IbcPacket,
    err: String,
) -> Result<IbcBasicResponse, ContractError> {
    let msg: Ics20Packet = from_binary(&packet.data)?;

    // in case that the denom is not in the mapping list, meaning that it is not transferred back, but transfer originally from this local chain
    if ics20_denoms().may_load(deps.storage, &msg.denom)?.is_none() {
        // undo the balance update on failure (as we pre-emptively added it on send)
        reduce_channel_balance(
            deps.storage,
            &packet.src.channel_id,
            &msg.denom,
            msg.amount,
            true,
        )?;

        let to_send = Amount::from_parts(msg.denom.clone(), msg.amount);
        let gas_limit = check_gas_limit(deps.as_ref(), &to_send)?;
        let send = send_amount(to_send, msg.sender.clone(), None);
        let mut submsg = SubMsg::reply_on_error(send, ACK_FAILURE_ID);
        submsg.gas_limit = gas_limit;

        // similar event messages like ibctransfer module
        let res = IbcBasicResponse::new()
            .add_submessage(submsg)
            .add_attribute("action", "acknowledge")
            .add_attribute("sender", msg.sender)
            .add_attribute("receiver", msg.receiver)
            .add_attribute("denom", msg.denom)
            .add_attribute("amount", msg.amount.to_string())
            .add_attribute("success", "false")
            .add_attribute("error", err);

        return Ok(res);
    }

    // since we reduce the channel's balance optimistically when transferring back, we increase it again when receiving failed ack
    increase_channel_balance(
        deps.storage,
        &packet.src.channel_id,
        &msg.denom,
        msg.amount,
        false,
    )?;

    // get ibc denom mapping to get cw20 denom & from decimals in case of packet failure, we can refund the corresponding user & amount
    let pair_mapping = ics20_denoms().load(deps.storage, &msg.denom)?;
    let to_send = Amount::from_parts(
        parse_asset_info_denom(pair_mapping.asset_info),
        convert_remote_to_local(
            msg.amount,
            pair_mapping.remote_decimals,
            pair_mapping.asset_info_decimals,
        )?,
    );
    let cosmos_msg = send_amount(to_send, msg.sender.clone(), None);
    let submsg = SubMsg::reply_on_error(cosmos_msg, ACK_FAILURE_ID);

    // used submsg here & reply on error. This means that if the refund process fails => tokens will be locked in this IBC Wasm contract. We will manually handle that case. No retry
    // similar event messages like ibctransfer module
    let res = IbcBasicResponse::new()
        .add_submessage(submsg)
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

pub fn send_amount(amount: Amount, recipient: String, msg: Option<Binary>) -> CosmosMsg {
    match amount {
        Amount::Native(coin) => BankMsg::Send {
            to_address: recipient,
            amount: vec![coin],
        }
        .into(),
        Amount::Cw20(coin) => {
            let mut msg_cw20 = Cw20ExecuteMsg::Transfer {
                recipient: recipient.clone(),
                amount: coin.amount,
            };
            if let Some(msg) = msg {
                msg_cw20 = Cw20ExecuteMsg::Send {
                    contract: recipient,
                    amount: coin.amount,
                    msg,
                };
            }
            WasmMsg::Execute {
                contract_addr: coin.address,
                msg: to_binary(&msg_cw20).unwrap(),
                funds: vec![],
            }
            .into()
        }
    }
}

pub fn parse_asset_info_denom(asset_info: AssetInfo) -> String {
    match asset_info {
        AssetInfo::Token { contract_addr } => format!("cw20:{}", contract_addr.to_string()),
        AssetInfo::NativeToken { denom } => denom,
    }
}

pub fn parse_ibc_wasm_port_id(contract_addr: String) -> String {
    format!("wasm.{}", contract_addr)
}
