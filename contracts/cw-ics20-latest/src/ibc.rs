use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Api, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env,
    IbcBasicResponse, IbcChannel, IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg,
    IbcEndpoint, IbcMsg, IbcOrder, IbcPacket, IbcPacketAckMsg, IbcPacketReceiveMsg,
    IbcPacketTimeoutMsg, IbcReceiveResponse, QuerierWrapper, Reply, Response, StdError, Storage,
    SubMsg, SubMsgResult, Uint128, WasmMsg,
};
use cw20_ics20_msg::receiver::DestinationInfo;
use oraiswap::asset::AssetInfo;
use oraiswap::router::SwapOperation;

use crate::error::{ContractError, Never};
use crate::state::{
    get_key_ics20_ibc_denom, ics20_denoms, increase_channel_balance, reduce_channel_balance,
    undo_increase_channel_balance, undo_reduce_channel_balance, ChannelInfo, ReplyArgs, ALLOW_LIST,
    CHANNEL_INFO, CONFIG, REPLY_ARGS,
};
use cw20::Cw20ExecuteMsg;
use cw20_ics20_msg::amount::{convert_remote_to_local, Amount};

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

const RECEIVE_ID: u64 = 1337;
const NATIVE_RECEIVE_ID: u64 = 1338;
const FOLLOW_UP_MSGS_ID: u64 = 1339;
const ACK_FAILURE_ID: u64 = 64023;
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
            SubMsgResult::Err(err) => Ok(Response::new()
                .set_data(ack_fail(err.clone()))
                .add_attribute("error_follow_up_msgs", err)),
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
    _env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    let packet = msg.packet;

    do_ibc_packet_receive(deps, &packet).or_else(|err| {
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
    let receiver: DestinationInfo = DestinationInfo::from_str(&msg.receiver);
    // after receiving the cw20 amount, we try to do fee swapping for the user if needed so he / she can create txs on the network
    let submsgs: Vec<SubMsg> =
        get_follow_up_msgs(storage, api, querier, to_send, &receiver, packet)?
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

// TODO: add unit & e2e tests for this function
fn get_follow_up_msgs(
    storage: &mut dyn Storage,
    api: &dyn Api,
    querier: &QuerierWrapper,
    to_send: Amount,
    receiver: &DestinationInfo,
    packet: &IbcPacket,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let config = CONFIG.load(storage)?;
    let is_channel_empty = receiver.destination_channel.is_empty();
    if receiver.destination_denom.is_empty() {
        if is_channel_empty {
            return Ok(vec![send_amount(to_send, receiver.receiver.clone(), None)]);
        }
        return Err(ContractError::Std(StdError::generic_err("Invalid destination info. Must have destination denom if there's a destination channel")));
    }
    // successful case. We dont care if this msg is going to be successful or not because it does not affect our ibc receive flow (just a submsg)
    let cw20_address = api.addr_validate(&to_send.raw_denom())?;
    let mut swap_operations = vec![SwapOperation::OraiSwap {
        offer_asset_info: AssetInfo::Token {
            contract_addr: cw20_address.clone(),
        },
        ask_asset_info: AssetInfo::NativeToken {
            denom: config.fee_denom.clone(),
        },
    }];
    // config.fee_denom is likely to be ORAI, we can use it to deduct relayer fee. It is also used to confirm multiple swap ops
    if !receiver.destination_denom.eq(&config.fee_denom) {
        // if we can parse the denom into a valid orai address => it is a cw20 contract addr, otherwise it is native token
        swap_operations.push(SwapOperation::OraiSwap {
            offer_asset_info: AssetInfo::NativeToken {
                denom: config.fee_denom,
            },
            ask_asset_info: if let Some(contract_addr) =
                api.addr_validate(&receiver.destination_denom).ok()
            {
                AssetInfo::Token { contract_addr }
            } else {
                AssetInfo::NativeToken {
                    denom: receiver.destination_denom.clone(),
                }
            },
        });
    }
    let mut minimum_receive = None;
    let mut to: Option<String> = Some(receiver.receiver.clone());
    let mut ibc_msg: Option<CosmosMsg> = None;
    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];
    let swap_amount = to_send.amount();
    // if there's destination denom then it means we'll have to create a new ibc transfer msg after swapping => need to simulate swap to get receiving amount
    // another case is evm-prefix address, which will automatically be forwarded using the ibc transfer. For this case, it should have no channel, and receiver addr should not be in orai... format
    if !is_channel_empty || (is_channel_empty && api.addr_validate(&receiver.receiver).is_err()) {
        minimum_receive = Some(querier.query_wasm_smart(
            config.swap_router_contract.clone(),
            &oraiswap::router::QueryMsg::SimulateSwapOperations {
                offer_amount: swap_amount.clone(),
                operations: swap_operations.clone(),
            },
        )?);
        // 'to' is also adjusted to None, because this contract will receive the ask amount & send them using ibc
        to = None;

        // TODO: if receiver is in form of cosmos token then we create ibc transfer msg, else we create ibc wasm transfer msg for evm case
        if receiver.is_receiver_evm_based() {
        } else {
        }
    }
    cosmos_msgs.push(
        WasmMsg::Execute {
            contract_addr: cw20_address.into_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: config.swap_router_contract,
                amount: swap_amount,
                msg: to_binary(&oraiswap::router::Cw20HookMsg::ExecuteSwapOperations {
                    operations: swap_operations,
                    minimum_receive,
                    to,
                })?,
            })?,
            funds: vec![],
        }
        .into(),
    );
    if let Some(ibc_msg) = ibc_msg {
        cosmos_msgs.push(ibc_msg.into());
    }
    return Ok(cosmos_msgs);
}

fn check_gas_limit(deps: Deps, amount: &Amount) -> Result<Option<u64>, ContractError> {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::*;

    use crate::contract::{execute, migrate, query_channel};
    use crate::msg::{ExecuteMsg, MigrateMsg, TransferMsg, UpdatePairMsg};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, to_vec, Addr, Decimal, IbcEndpoint, IbcMsg, IbcTimeout, Timestamp};
    use cw20::Cw20ReceiveMsg;
    use oraiswap::asset::AssetInfo;

    #[test]
    fn check_ack_json() {
        let success = Ics20Ack::Result(b"1".into());
        let fail = Ics20Ack::Error("bad coin".into());

        let success_json = String::from_utf8(to_vec(&success).unwrap()).unwrap();
        assert_eq!(r#"{"result":"MQ=="}"#, success_json.as_str());

        let fail_json = String::from_utf8(to_vec(&fail).unwrap()).unwrap();
        assert_eq!(r#"{"error":"bad coin"}"#, fail_json.as_str());
    }

    #[test]
    fn check_packet_json() {
        let packet = Ics20Packet::new(
            Uint128::new(12345),
            "ucosm",
            "cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
            "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc",
            None,
        );
        // Example message generated from the SDK
        let expected = r#"{"amount":"12345","denom":"ucosm","receiver":"wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc","sender":"cosmos1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","memo":null}"#;

        let encdoded = String::from_utf8(to_vec(&packet).unwrap()).unwrap();
        assert_eq!(expected, encdoded.as_str());
    }

    fn cw20_payment(
        amount: u128,
        address: &str,
        recipient: &str,
        gas_limit: Option<u64>,
    ) -> SubMsg {
        let msg = Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: Uint128::new(amount),
        };
        let exec = WasmMsg::Execute {
            contract_addr: address.into(),
            msg: to_binary(&msg).unwrap(),
            funds: vec![],
        };
        let mut msg = SubMsg::reply_on_error(exec, RECEIVE_ID);
        msg.gas_limit = gas_limit;
        msg
    }

    fn native_payment(amount: u128, denom: &str, recipient: &str) -> SubMsg {
        SubMsg::reply_on_error(
            BankMsg::Send {
                to_address: recipient.into(),
                amount: coins(amount, denom),
            },
            RECEIVE_ID,
        )
    }

    fn mock_receive_packet(
        my_channel: &str,
        amount: u128,
        denom: &str,
        receiver: &str,
    ) -> IbcPacket {
        let data = Ics20Packet {
            // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
            denom: format!("{}/{}/{}", REMOTE_PORT, "channel-1234", denom),
            amount: amount.into(),
            sender: "remote-sender".to_string(),
            receiver: receiver.to_string(),
            memo: None,
        };
        IbcPacket::new(
            to_binary(&data).unwrap(),
            IbcEndpoint {
                port_id: REMOTE_PORT.to_string(),
                channel_id: "channel-1234".to_string(),
            },
            IbcEndpoint {
                port_id: CONTRACT_PORT.to_string(),
                channel_id: my_channel.to_string(),
            },
            3,
            Timestamp::from_seconds(1665321069).into(),
        )
    }

    #[test]
    fn send_receive_cw20() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let cw20_denom = "cw20:token-addr";
        let gas_limit = 1234567;
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );

        // prepare some mock packets
        let recv_packet = mock_receive_packet(send_channel, 876543210, cw20_denom, "local-rcpt");
        let recv_high_packet =
            mock_receive_packet(send_channel, 1876543210, cw20_denom, "local-rcpt");

        // cannot receive this denom yet
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        let no_funds = Ics20Ack::Error(ContractError::InsufficientFunds {}.to_string());
        assert_eq!(ack, no_funds);

        // we send some cw20 tokens over
        let transfer = TransferMsg {
            channel: send_channel.to_string(),
            remote_address: "remote-rcpt".to_string(),
            timeout: None,
            memo: None,
        };
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "local-sender".to_string(),
            amount: Uint128::new(987654321),
            msg: to_binary(&transfer).unwrap(),
        });
        let info = mock_info(cw20_addr, &[]);
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
        let expected = Ics20Packet {
            denom: cw20_denom.into(),
            amount: Uint128::new(987654321),
            sender: "local-sender".to_string(),
            receiver: "remote-rcpt".to_string(),
            memo: None,
        };
        let timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
        assert_eq!(
            &res.messages[0],
            &SubMsg::new(IbcMsg::SendPacket {
                channel_id: send_channel.to_string(),
                data: to_binary(&expected).unwrap(),
                timeout: IbcTimeout::with_timestamp(timeout),
            })
        );

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::cw20(987654321, cw20_addr)]);
        assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);

        // cannot receive more than we sent
        let msg = IbcPacketReceiveMsg::new(recv_high_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert_eq!(ack, no_funds);

        // we can receive less than we sent
        let msg = IbcPacketReceiveMsg::new(recv_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert_eq!(1, res.messages.len());
        assert_eq!(
            cw20_payment(876543210, cw20_addr, "local-rcpt", Some(gas_limit)),
            res.messages[0]
        );
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // query channel state
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::cw20(111111111, cw20_addr)]);
        assert_eq!(state.total_sent, vec![Amount::cw20(987654321, cw20_addr)]);
    }

    #[test]
    fn send_receive_native() {
        let send_channel = "channel-9";
        let mut deps = setup(&["channel-1", "channel-7", send_channel], &[]);

        let denom = "uatom";

        // prepare some mock packets
        let recv_packet = mock_receive_packet(send_channel, 876543210, denom, "local-rcpt");
        let recv_high_packet = mock_receive_packet(send_channel, 1876543210, denom, "local-rcpt");

        // cannot receive this denom yet
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        let no_funds = Ics20Ack::Error(ContractError::InsufficientFunds {}.to_string());
        assert_eq!(ack, no_funds);

        // we transfer some tokens
        let msg = ExecuteMsg::Transfer(TransferMsg {
            channel: send_channel.to_string(),
            remote_address: "my-remote-address".to_string(),
            timeout: None,
            memo: Some("memo".to_string()),
        });
        let info = mock_info("local-sender", &coins(987654321, denom));
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::native(987654321, denom)]);
        assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);

        // cannot receive more than we sent
        let msg = IbcPacketReceiveMsg::new(recv_high_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert!(res.messages.is_empty());
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert_eq!(ack, no_funds);

        // we can receive less than we sent
        let msg = IbcPacketReceiveMsg::new(recv_packet);
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        assert_eq!(1, res.messages.len());
        assert_eq!(
            native_payment(876543210, denom, "local-rcpt"),
            res.messages[0]
        );
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // only need to call reply block on error case

        // query channel state
        let state = query_channel(deps.as_ref(), send_channel.to_string(), Some(true)).unwrap();
        assert_eq!(state.balances, vec![Amount::native(111111111, denom)]);
        assert_eq!(state.total_sent, vec![Amount::native(987654321, denom)]);
    }

    #[test]
    fn check_gas_limit_handles_all_cases() {
        let send_channel = "channel-9";
        let allowed = "foobar";
        let allowed_gas = 777666;
        let mut deps = setup(&[send_channel], &[(allowed, allowed_gas)]);

        // allow list will get proper gas
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
        assert_eq!(limit, Some(allowed_gas));

        // non-allow list will error
        let random = "tokenz";
        check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap_err();

        // add default_gas_limit
        let def_limit = 54321;
        migrate(
            deps.as_mut(),
            mock_env(),
            MigrateMsg {
                default_gas_limit: Some(def_limit),
                default_timeout: 100u64,
                default_orai_fee_swap: Decimal::percent(5),
                fee_denom: "orai".to_string(),
                swap_router_contract: "foobar".to_string(),
            },
        )
        .unwrap();

        // allow list still gets proper gas
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, allowed)).unwrap();
        assert_eq!(limit, Some(allowed_gas));

        // non-allow list will now get default
        let limit = check_gas_limit(deps.as_ref(), &Amount::cw20(500, random)).unwrap();
        assert_eq!(limit, Some(def_limit));
    }

    // test remote chain send native token to local chain
    fn mock_receive_packet_remote_to_local(
        my_channel: &str,
        amount: u128,
        denom: &str,
        receiver: &str,
    ) -> IbcPacket {
        let data = Ics20Packet {
            // this is returning a foreign native token, thus denom is <denom>, eg: uatom
            denom: format!("{}", denom),
            amount: amount.into(),
            sender: "remote-sender".to_string(),
            receiver: receiver.to_string(),
            memo: None,
        };
        IbcPacket::new(
            to_binary(&data).unwrap(),
            IbcEndpoint {
                port_id: REMOTE_PORT.to_string(),
                channel_id: "channel-1234".to_string(),
            },
            IbcEndpoint {
                port_id: CONTRACT_PORT.to_string(),
                channel_id: my_channel.to_string(),
            },
            3,
            Timestamp::from_seconds(1665321069).into(),
        )
    }

    #[test]
    fn test_parse_voucher_denom_invalid_length() {
        let voucher_denom = "foobar/foobar";
        let ibc_endpoint = IbcEndpoint {
            port_id: "hello".to_string(),
            channel_id: "world".to_string(),
        };
        // native denom case
        assert_eq!(
            parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
            ContractError::NoForeignTokens {}
        );
    }

    #[test]
    fn test_parse_voucher_denom_invalid_port() {
        let voucher_denom = "foobar/abc/xyz";
        let ibc_endpoint = IbcEndpoint {
            port_id: "hello".to_string(),
            channel_id: "world".to_string(),
        };
        // native denom case
        assert_eq!(
            parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
            ContractError::FromOtherPort {
                port: "foobar".to_string()
            }
        );
    }

    #[test]
    fn test_parse_voucher_denom_invalid_channel() {
        let voucher_denom = "hello/abc/xyz";
        let ibc_endpoint = IbcEndpoint {
            port_id: "hello".to_string(),
            channel_id: "world".to_string(),
        };
        // native denom case
        assert_eq!(
            parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap_err(),
            ContractError::FromOtherChannel {
                channel: "abc".to_string()
            }
        );
    }

    #[test]
    fn test_parse_voucher_denom_native_denom_valid() {
        let voucher_denom = "foobar";
        let ibc_endpoint = IbcEndpoint {
            port_id: "hello".to_string(),
            channel_id: "world".to_string(),
        };
        assert_eq!(
            parse_voucher_denom(voucher_denom, &ibc_endpoint).unwrap(),
            ("foobar", true)
        );
    }

    /////////////////////////////// Test cases for native denom transfer from remote chain to local chain

    #[test]
    fn send_native_from_remote_mapping_not_found() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let custom_addr = "custom-addr";
        let cw20_denom = "cw20:token-addr";
        let gas_limit = 1234567;
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );

        // prepare some mock packets
        let recv_packet =
            mock_receive_packet_remote_to_local(send_channel, 876543210, cw20_denom, custom_addr);

        // we can receive this denom, channel balance should increase
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        // assert_eq!(res, StdError)
        assert_eq!(
            res.attributes.last().unwrap().value,
            "You can only send native tokens that has a map to the corresponding asset info"
        );
    }

    #[test]
    fn send_native_from_remote_receive_happy_path() {
        let send_channel = "channel-9";
        let cw20_addr = "token-addr";
        let custom_addr = "custom-addr";
        let denom = "uatom";
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:token-addr"),
        };
        let gas_limit = 1234567;
        let mut deps = setup(
            &["channel-1", "channel-7", send_channel],
            &[(cw20_addr, gas_limit)],
        );

        let pair = UpdatePairMsg {
            local_channel_id: send_channel.to_string(),
            denom: denom.to_string(),
            asset_info: asset_info,
            remote_decimals: 18u8,
            asset_info_decimals: 18u8,
        };

        let _ = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("gov", &[]),
            ExecuteMsg::UpdateMappingPair(pair),
        )
        .unwrap();

        // prepare some mock packets
        let recv_packet =
            mock_receive_packet_remote_to_local(send_channel, 876543210, denom, custom_addr);

        // we can receive this denom, channel balance should increase
        let msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        let res = ibc_packet_receive(deps.as_mut(), mock_env(), msg).unwrap();
        println!("res: {:?}", res);
        assert_eq!(res.messages.len(), 2);
        let ack: Ics20Ack = from_binary(&res.acknowledgement).unwrap();
        println!("ack: {:?}", ack);
        assert!(matches!(ack, Ics20Ack::Result(_)));

        // query channel state|_|
        let state = query_channel(deps.as_ref(), send_channel.to_string(), None).unwrap();
        assert_eq!(
            state.balances,
            vec![Amount::native(
                876543210,
                &get_key_ics20_ibc_denom(CONTRACT_PORT, send_channel, denom)
            )]
        );
        assert_eq!(
            state.total_sent,
            vec![Amount::native(
                876543210,
                &get_key_ics20_ibc_denom(CONTRACT_PORT, send_channel, denom)
            )]
        );
    }
}
