#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcEndpoint, IbcMsg, IbcQuery,
    MessageInfo, Order, PortIdResponse, Response, StdResult, Storage,
};
use cw2::set_contract_version;
use cw20::{Cw20Coin, Cw20ReceiveMsg};
use cw_storage_plus::Bound;
use oraiswap::asset::AssetInfo;
use oraiswap::router::RouterController;

use crate::error::ContractError;
use crate::ibc::{
    collect_transfer_fee_msgs, parse_ibc_wasm_port_id, parse_voucher_denom, process_deduct_fee,
    Ics20Packet,
};
use crate::msg::{
    AllowMsg, AllowedInfo, AllowedResponse, ChannelResponse, ConfigResponse, DeletePairMsg,
    ExecuteMsg, InitMsg, ListAllowedResponse, ListChannelsResponse, ListMappingResponse,
    MigrateMsg, PairQuery, PortResponse, QueryMsg, TransferBackMsg, UpdatePairMsg,
};
use crate::state::{
    get_key_ics20_ibc_denom, ics20_denoms, AllowInfo, Config, MappingMetadata, RelayerFee,
    TokenFee, ADMIN, ALLOW_LIST, CHANNEL_INFO, CHANNEL_REVERSE_STATE, CONFIG, RELAYER_FEE,
    TOKEN_FEE,
};
use cw20_ics20_msg::amount::{convert_local_to_remote, Amount};
use cw_utils::{maybe_addr, nonpayable, one_coin};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-ics20";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InitMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let admin = deps.api.addr_validate(&msg.gov_contract)?;
    ADMIN.set(deps.branch(), Some(admin.clone()))?;
    let cfg = Config {
        default_timeout: msg.default_timeout,
        default_gas_limit: msg.default_gas_limit,
        fee_denom: "orai".to_string(),
        swap_router_contract: RouterController(msg.swap_router_contract),
        fee_receiver: admin,
    };
    CONFIG.save(deps.storage, &cfg)?;

    // add all allows
    for allowed in msg.allowlist {
        let contract = deps.api.addr_validate(&allowed.contract)?;
        let info = AllowInfo {
            gas_limit: allowed.gas_limit,
        };
        ALLOW_LIST.save(deps.storage, &contract, &info)?;
    }
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        // ExecuteMsg::Transfer(msg) => {
        //     let coin = one_coin(&info)?;
        //     execute_transfer(deps, env, msg, Amount::Native(coin), info.sender)
        // }
        ExecuteMsg::TransferToRemote(msg) => {
            let coin = one_coin(&info)?;
            let amount = Amount::from_parts(coin.denom, coin.amount);
            execute_transfer_back_to_remote_chain(deps, env, msg, amount, info.sender)
        }
        ExecuteMsg::UpdateMappingPair(msg) => execute_update_mapping_pair(deps, env, info, msg),
        ExecuteMsg::DeleteMappingPair(msg) => execute_delete_mapping_pair(deps, env, info, msg),
        ExecuteMsg::Allow(allow) => execute_allow(deps, env, info, allow),
        ExecuteMsg::UpdateConfig {
            default_timeout,
            default_gas_limit,
            fee_denom,
            swap_router_contract,
            admin,
            token_fee,
            fee_receiver,
            relayer_fee,
        } => update_config(
            deps,
            info,
            default_timeout,
            default_gas_limit,
            fee_denom,
            swap_router_contract,
            admin,
            token_fee,
            fee_receiver,
            relayer_fee,
        ),
    }
}

pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    default_timeout: Option<u64>,
    default_gas_limit: Option<u64>,
    fee_denom: Option<String>,
    swap_router_contract: Option<String>,
    admin: Option<String>,
    token_fee: Option<Vec<TokenFee>>,
    fee_receiver: Option<String>,
    relayer_fee: Option<Vec<RelayerFee>>,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;
    if let Some(token_fee) = token_fee {
        for fee in token_fee {
            TOKEN_FEE.save(deps.storage, &fee.token_denom, &fee.ratio)?;
        }
    }
    if let Some(relayer_fee) = relayer_fee {
        for fee in relayer_fee {
            RELAYER_FEE.save(deps.storage, &fee.prefix, &fee.fee)?;
        }
    }
    CONFIG.update(deps.storage, |mut config| -> StdResult<Config> {
        if let Some(default_timeout) = default_timeout {
            config.default_timeout = default_timeout;
        }
        if let Some(fee_denom) = fee_denom {
            config.fee_denom = fee_denom;
        }
        if let Some(swap_router_contract) = swap_router_contract {
            config.swap_router_contract = RouterController(swap_router_contract);
        }
        if let Some(fee_receiver) = fee_receiver {
            config.fee_receiver = deps.api.addr_validate(&fee_receiver)?;
        }
        config.default_gas_limit = default_gas_limit;
        Ok(config)
    })?;
    if let Some(admin) = admin {
        let admin = deps.api.addr_validate(&admin)?;
        ADMIN.execute_update_admin::<Empty, Empty>(deps, info, Some(admin))?;
    }
    Ok(Response::default().add_attribute("action", "update_config"))
}

pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    let amount = Amount::Cw20(Cw20Coin {
        address: info.sender.to_string(),
        amount: wrapper.amount,
    });
    let api = deps.api;

    // let msg_result: StdResult<TransferMsg> = from_binary(&wrapper.msg);
    // if msg_result.is_ok() {
    //     let msg: TransferMsg = msg_result.unwrap();
    //     return execute_transfer(deps, env, msg, amount, api.addr_validate(&wrapper.sender)?);
    // }

    let msg: TransferBackMsg = from_binary(&wrapper.msg)?;
    execute_transfer_back_to_remote_chain(
        deps,
        env,
        msg,
        amount,
        api.addr_validate(&wrapper.sender)?,
    )
}

// pub fn execute_transfer(
//     deps: DepsMut,
//     env: Env,
//     msg: TransferMsg,
//     amount: Amount,
//     sender: Addr,
// ) -> Result<Response, ContractError> {
//     if amount.is_empty() {
//         return Err(ContractError::NoFunds {});
//     }
//     // ensure the requested channel is registered
//     if !CHANNEL_INFO.has(deps.storage, &msg.channel) {
//         return Err(ContractError::NoSuchChannel { id: msg.channel });
//     }
//     let config = CONFIG.load(deps.storage)?;

//     // if cw20 token, validate and ensure it is whitelisted, or we set default gas limit
//     if let Amount::Cw20(coin) = &amount {
//         let addr = deps.api.addr_validate(&coin.address)?;
//         // if limit is set, then we always allow cw20
//         if config.default_gas_limit.is_none() {
//             ALLOW_LIST
//                 .may_load(deps.storage, &addr)?
//                 .ok_or(ContractError::NotOnAllowList)?;
//         }
//     };

//     // delta from user is in seconds
//     let timeout_delta = match msg.timeout {
//         Some(t) => t,
//         None => config.default_timeout,
//     };
//     // timeout is in nanoseconds
//     let timeout = env.block.time.plus_seconds(timeout_delta);

//     // build ics20 packet
//     let packet = Ics20Packet::new(
//         amount.amount(),
//         amount.denom(),
//         sender.as_ref(),
//         &msg.remote_address,
//         msg.memo,
//     );
//     packet.validate()?;

//     // Update the balance now (optimistically) like ibctransfer modules.
//     // In on_packet_failure (ack with error message or a timeout), we reduce the balance appropriately.
//     // This means the channel works fine if success acks are not relayed.
//     increase_channel_balance(
//         deps.storage,
//         &msg.channel,
//         &amount.denom(),
//         amount.amount(),
//         true,
//     )?;

//     // prepare ibc message
//     let msg = IbcMsg::SendPacket {
//         channel_id: msg.channel,
//         data: to_binary(&packet)?,
//         timeout: timeout.into(),
//     };

//     // send response
//     let res = Response::new()
//         .add_message(msg)
//         .add_attribute("action", "transfer")
//         .add_attribute("sender", &packet.sender)
//         .add_attribute("receiver", &packet.receiver)
//         .add_attribute("denom", &packet.denom)
//         .add_attribute("amount", &packet.amount.to_string());
//     Ok(res)
// }

pub fn execute_transfer_back_to_remote_chain(
    deps: DepsMut,
    env: Env,
    msg: TransferBackMsg,
    amount: Amount,
    sender: Addr,
) -> Result<Response, ContractError> {
    if amount.is_empty() {
        return Err(ContractError::NoFunds {});
    }
    let config = CONFIG.load(deps.storage)?;

    // should be in form port/channel/denom
    let mappings = get_mappings_from_asset_info(
        deps.as_ref().storage,
        match amount.clone() {
            Amount::Native(coin) => AssetInfo::NativeToken { denom: coin.denom },
            Amount::Cw20(cw20_coin) => AssetInfo::Token {
                contract_addr: deps.api.addr_validate(cw20_coin.address.as_str())?,
            },
        },
    )?;

    // parse denom & compare with user input. Should not use string.includes() because hacker can fake a port that has the same remote denom to return true
    let mapping = mappings
        .into_iter()
        .find(|pair| -> bool {
            let (denom, is_native) = parse_voucher_denom(
                pair.key.as_str(),
                &IbcEndpoint {
                    port_id: parse_ibc_wasm_port_id(env.contract.address.clone().into_string()),
                    channel_id: msg.local_channel_id.clone(), // also verify local channel id
                },
            )
            .unwrap_or_else(|_| ("", true)); // if there's an error, change is_native to true so it automatically returns false
            if is_native {
                return false;
            }
            if denom.eq(&msg.remote_denom) {
                return true;
            }
            return false;
        })
        .ok_or(ContractError::MappingPairNotFound {})?;

    // if found mapping, then deduct fee based on mapping
    let new_deducted_amount = process_deduct_fee(
        deps.storage,
        &deps.querier,
        &msg.remote_address,
        &msg.remote_denom,
        amount.amount(),
        mapping.pair_mapping.asset_info_decimals,
        &amount.denom(),
        &config.swap_router_contract,
    )?;

    let ibc_denom = mapping.key;
    // ensure the requested channel is registered
    if !CHANNEL_INFO.has(deps.storage, &msg.local_channel_id) {
        return Err(ContractError::NoSuchChannel {
            id: msg.local_channel_id,
        });
    }

    // delta from user is in seconds
    let timeout_delta = match msg.timeout {
        Some(t) => t,
        None => config.default_timeout,
    };
    // timeout is in nanoseconds
    let timeout = env.block.time.plus_seconds(timeout_delta);
    // need to convert decimal of cw20 to remote decimal before transferring
    let amount_remote = convert_local_to_remote(
        new_deducted_amount,
        mapping.pair_mapping.remote_decimals,
        mapping.pair_mapping.asset_info_decimals,
    )?;

    // build ics20 packet
    let packet = Ics20Packet::new(
        amount_remote.clone(),
        ibc_denom.clone(), // we use ibc denom in form <transfer>/<channel>/<denom> so that when it is sent back to remote chain, it gets parsed correctly and burned
        sender.as_str(),
        &msg.remote_address,
        msg.memo,
    );
    packet.validate()?;

    // now this is processed in ack
    // // because we are transferring back, we reduce the channel's balance
    // reduce_channel_balance(
    //     deps.storage,
    //     &msg.local_channel_id,
    //     &ibc_denom,
    //     amount_remote,
    //     false,
    // )?;

    // prepare ibc message
    let msg = IbcMsg::SendPacket {
        channel_id: msg.local_channel_id,
        data: to_binary(&packet)?,
        timeout: timeout.into(),
    };

    let mut cosmos_msgs =
        collect_transfer_fee_msgs(config.fee_receiver.into_string(), deps.storage)?;
    cosmos_msgs.push(msg.into());

    // send response
    let res = Response::new()
        .add_messages(cosmos_msgs)
        .add_attribute("action", "transfer")
        .add_attribute("type", "transfer_back_to_remote_chain")
        .add_attribute("sender", &packet.sender)
        .add_attribute("receiver", &packet.receiver)
        .add_attribute("denom", &packet.denom)
        .add_attribute("amount", &packet.amount.to_string());
    Ok(res)
}

/// The gov contract can allow new contracts, or increase the gas limit on existing contracts.
/// It cannot block or reduce the limit to avoid forcible sticking tokens in the channel.
pub fn execute_allow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    allow: AllowMsg,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;

    let contract = deps.api.addr_validate(&allow.contract)?;
    let set = AllowInfo {
        gas_limit: allow.gas_limit,
    };
    ALLOW_LIST.update(deps.storage, &contract, |old| {
        if let Some(old) = old {
            // we must ensure it increases the limit
            match (old.gas_limit, set.gas_limit) {
                (None, Some(_)) => return Err(ContractError::CannotLowerGas),
                (Some(old), Some(new)) if new < old => return Err(ContractError::CannotLowerGas),
                _ => {}
            };
        }
        Ok(AllowInfo {
            gas_limit: allow.gas_limit,
        })
    })?;

    let gas = if let Some(gas) = allow.gas_limit {
        gas.to_string()
    } else {
        "None".to_string()
    };

    let res = Response::new()
        .add_attribute("action", "allow")
        .add_attribute("contract", allow.contract)
        .add_attribute("gas_limit", gas);
    Ok(res)
}

/// The gov contract can allow new contracts, or increase the gas limit on existing contracts.
/// It cannot block or reduce the limit to avoid forcible sticking tokens in the channel.
pub fn execute_update_mapping_pair(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mapping_pair_msg: UpdatePairMsg,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;

    let ibc_denom = get_key_ics20_ibc_denom(
        &parse_ibc_wasm_port_id(env.contract.address.into_string()),
        &mapping_pair_msg.local_channel_id,
        &mapping_pair_msg.denom,
    );

    // if pair already exists in list, remove it and create a new one
    if ics20_denoms().load(deps.storage, &ibc_denom).is_ok() {
        ics20_denoms().remove(deps.storage, &ibc_denom)?;
    }

    ics20_denoms().save(
        deps.storage,
        &ibc_denom,
        &MappingMetadata {
            asset_info: mapping_pair_msg.asset_info.clone(),
            remote_decimals: mapping_pair_msg.remote_decimals,
            asset_info_decimals: mapping_pair_msg.asset_info_decimals,
        },
    )?;

    let res = Response::new()
        .add_attribute("action", "execute_update_mapping_pair")
        .add_attribute("denom", mapping_pair_msg.denom)
        .add_attribute("new_asset_info", mapping_pair_msg.asset_info.to_string());
    Ok(res)
}

pub fn execute_delete_mapping_pair(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mapping_pair_msg: DeletePairMsg,
) -> Result<Response, ContractError> {
    ADMIN.assert_admin(deps.as_ref(), &info.sender)?;

    let ibc_denom = get_key_ics20_ibc_denom(
        &parse_ibc_wasm_port_id(env.contract.address.into_string()),
        &mapping_pair_msg.local_channel_id,
        &mapping_pair_msg.denom,
    );

    ics20_denoms().remove(deps.storage, &ibc_denom)?;

    let res = Response::new()
        .add_attribute("action", "execute_delete_mapping_pair")
        .add_attribute("local_channel_id", mapping_pair_msg.local_channel_id)
        .add_attribute("original_denom", mapping_pair_msg.denom);
    Ok(res)
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    // we don't need to save anything if migrating from the same version
    CONFIG.save(
        deps.storage,
        &Config {
            default_timeout: msg.default_timeout,
            default_gas_limit: msg.default_gas_limit,
            fee_denom: msg.fee_denom,
            swap_router_contract: RouterController(msg.swap_router_contract),
            fee_receiver: deps.api.addr_validate(&msg.fee_receiver)?,
        },
    )?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Port {} => to_binary(&query_port(deps)?),
        QueryMsg::ListChannels {} => to_binary(&query_list(deps)?),
        QueryMsg::Channel { id, forward } => to_binary(&query_channel(deps, id, forward)?),
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Allowed { contract } => to_binary(&query_allowed(deps, contract)?),
        QueryMsg::ListAllowed {
            start_after,
            limit,
            order,
        } => to_binary(&list_allowed(deps, start_after, limit, order)?),
        QueryMsg::PairMappings {
            start_after,
            limit,
            order,
        } => to_binary(&list_cw20_mapping(deps, start_after, limit, order)?),
        QueryMsg::PairMapping { key } => to_binary(&get_mapping_from_key(deps, key)?),
        QueryMsg::PairMappingsFromAssetInfo { asset_info } => {
            to_binary(&get_mappings_from_asset_info(deps.storage, asset_info)?)
        }
        QueryMsg::Admin {} => to_binary(&ADMIN.query_admin(deps)?),
        QueryMsg::GetTransferTokenFee { remote_token_denom } => {
            to_binary(&TOKEN_FEE.load(deps.storage, &remote_token_denom)?)
        }
    }
}

fn query_port(deps: Deps) -> StdResult<PortResponse> {
    let query = IbcQuery::PortId {}.into();
    let PortIdResponse { port_id } = deps.querier.query(&query)?;
    Ok(PortResponse { port_id })
}

fn query_list(deps: Deps) -> StdResult<ListChannelsResponse> {
    let channels = CHANNEL_INFO
        .range_raw(deps.storage, None, None, Order::Ascending)
        .map(|r| r.map(|(_, v)| v))
        .collect::<StdResult<_>>()?;
    Ok(ListChannelsResponse { channels })
}

// make public for ibc tests
pub fn query_channel(deps: Deps, id: String, _forward: Option<bool>) -> StdResult<ChannelResponse> {
    let info = CHANNEL_INFO.load(deps.storage, &id)?;
    // this returns Vec<(outstanding, total)>
    let channel_state = CHANNEL_REVERSE_STATE;
    let state = channel_state
        .prefix(&id)
        .range(deps.storage, None, None, Order::Ascending)
        .map(|r| {
            // this denom is
            r.map(|(denom, v)| {
                let outstanding = Amount::from_parts(denom.clone(), v.outstanding);
                let total = Amount::from_parts(denom, v.total_sent);
                (outstanding, total)
            })
        })
        .collect::<StdResult<Vec<_>>>()?;
    // we want (Vec<outstanding>, Vec<total>)
    let (balances, total_sent): (Vec<Amount>, Vec<Amount>) = state.into_iter().unzip();

    Ok(ChannelResponse {
        info,
        balances,
        total_sent,
    })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let admin = ADMIN.get(deps)?.unwrap_or_else(|| Addr::unchecked(""));
    let res = ConfigResponse {
        default_timeout: cfg.default_timeout,
        default_gas_limit: cfg.default_gas_limit,
        fee_denom: cfg.fee_denom,
        swap_router_contract: cfg.swap_router_contract.addr(),
        gov_contract: admin.into(),
    };
    Ok(res)
}

fn query_allowed(deps: Deps, contract: String) -> StdResult<AllowedResponse> {
    let addr = deps.api.addr_validate(&contract)?;
    let info = ALLOW_LIST.may_load(deps.storage, &addr)?;
    let res = match info {
        None => AllowedResponse {
            is_allowed: false,
            gas_limit: None,
        },
        Some(a) => AllowedResponse {
            is_allowed: true,
            gas_limit: a.gas_limit,
        },
    };
    Ok(res)
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn list_allowed(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    order: Option<u8>,
) -> StdResult<ListAllowedResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.as_ref().map(Bound::exclusive);

    let allow = ALLOW_LIST
        .range(deps.storage, start, None, map_order(order))
        .take(limit)
        .map(|item| {
            item.map(|(addr, allow)| AllowedInfo {
                contract: addr.into(),
                gas_limit: allow.gas_limit,
            })
        })
        .collect::<StdResult<_>>()?;
    Ok(ListAllowedResponse { allow })
}

fn list_cw20_mapping(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    order: Option<u8>,
) -> StdResult<ListMappingResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let mut allow_range = ics20_denoms().range(deps.storage, None, None, map_order(order));
    if let Some(start_after) = start_after {
        let start = Some(Bound::exclusive::<&str>(&start_after));
        allow_range = ics20_denoms().range(deps.storage, start, None, map_order(order));
    }
    let pairs = allow_range
        .take(limit)
        .map(|item| {
            item.map(|(key, mapping)| PairQuery {
                key,
                pair_mapping: mapping,
            })
        })
        .collect::<StdResult<_>>()?;
    Ok(ListMappingResponse { pairs })
}

fn get_mapping_from_key(deps: Deps, ibc_denom: String) -> StdResult<PairQuery> {
    let result = ics20_denoms().load(deps.storage, &ibc_denom)?;
    Ok(PairQuery {
        key: ibc_denom,
        pair_mapping: result,
    })
}

fn get_mappings_from_asset_info(
    storage: &dyn Storage,
    asset_info: AssetInfo,
) -> StdResult<Vec<PairQuery>> {
    let pair_mapping_result: StdResult<Vec<(String, MappingMetadata)>> = ics20_denoms()
        .idx
        .asset_info
        .prefix(asset_info.to_string())
        .range(storage, None, None, Order::Ascending)
        .collect();
    if pair_mapping_result.is_err() {
        return Err(pair_mapping_result.unwrap_err());
    }
    let pair_mappings = pair_mapping_result.unwrap();
    let pair_queries: Vec<PairQuery> = pair_mappings
        .into_iter()
        .map(|pair| PairQuery {
            key: pair.0,
            pair_mapping: pair.1,
        })
        .collect();
    Ok(pair_queries)
}

fn map_order(order: Option<u8>) -> Order {
    if order.is_none() {
        return Order::Ascending;
    }
    let order_unwrap = order.unwrap();
    if order_unwrap == 1 {
        return Order::Ascending;
    }
    Order::Descending
}

#[cfg(test)]
mod test {
    use std::ops::Sub;

    use super::*;
    use crate::ibc::ibc_packet_receive;
    use crate::state::{reduce_channel_balance, Ratio};
    use crate::test_helpers::*;

    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{
        coins, CosmosMsg, Decimal, IbcEndpoint, IbcMsg, IbcPacket, IbcPacketReceiveMsg, StdError,
        Timestamp, Uint128, WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;
    use cw_controllers::AdminError;

    use oraiswap::asset::AssetInfo;

    #[test]
    fn test_split_denom() {
        let split_denom: Vec<&str> = "orai".splitn(3, '/').collect();
        assert_eq!(split_denom.len(), 1);

        let split_denom: Vec<&str> = "a/b/c".splitn(3, '/').collect();
        assert_eq!(split_denom.len(), 3)
    }

    #[test]
    fn setup_and_query() {
        let deps = setup(&["channel-3", "channel-7"], &[]);

        let raw_list = query(deps.as_ref(), mock_env(), QueryMsg::ListChannels {}).unwrap();
        let list_res: ListChannelsResponse = from_binary(&raw_list).unwrap();
        assert_eq!(2, list_res.channels.len());
        assert_eq!(mock_channel_info("channel-3"), list_res.channels[0]);
        assert_eq!(mock_channel_info("channel-7"), list_res.channels[1]);

        let raw_channel = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Channel {
                id: "channel-3".to_string(),
                forward: Some(true),
            },
        )
        .unwrap();
        let chan_res: ChannelResponse = from_binary(&raw_channel).unwrap();
        assert_eq!(chan_res.info, mock_channel_info("channel-3"));
        assert_eq!(0, chan_res.total_sent.len());
        assert_eq!(0, chan_res.balances.len());

        let err = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Channel {
                id: "channel-10".to_string(),
                forward: Some(true),
            },
        )
        .unwrap_err();
        assert_eq!(err, StdError::not_found("cw_ics20::state::ChannelInfo"));
    }

    #[test]
    fn test_query_pair_mapping_by_asset_info() {
        let mut deps = setup(&["channel-3", "channel-7"], &[]);
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:foobar".to_string()),
        };
        let mut update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: "earth".to_string(),
            asset_info: asset_info.clone(),
            remote_decimals: 18,
            asset_info_decimals: 18,
        };

        // works with proper funds
        let mut msg = ExecuteMsg::UpdateMappingPair(update.clone());

        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        // add another pair with the same asset info to filter
        update.denom = "jupiter".to_string();
        msg = ExecuteMsg::UpdateMappingPair(update.clone());
        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        // add another pair with a different asset info
        update.denom = "moon".to_string();
        update.asset_info = AssetInfo::NativeToken {
            denom: "orai".to_string(),
        };
        msg = ExecuteMsg::UpdateMappingPair(update.clone());
        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        // query based on asset info

        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappingsFromAssetInfo {
                asset_info: asset_info,
            },
        )
        .unwrap();
        let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
        assert_eq!(response.len(), 2);

        // query native token asset info, should receive moon denom in key
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappingsFromAssetInfo {
                asset_info: AssetInfo::NativeToken {
                    denom: "orai".to_string(),
                },
            },
        )
        .unwrap();
        let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
        assert_eq!(response.len(), 1);
        assert_eq!(response.first().unwrap().key.contains("moon"), true);

        // query asset info that is not in the mapping, should return empty
        // query native token asset info, should receive moon denom
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappingsFromAssetInfo {
                asset_info: AssetInfo::NativeToken {
                    denom: "foobar".to_string(),
                },
            },
        )
        .unwrap();
        let response: Vec<PairQuery> = from_binary(&mappings).unwrap();
        assert_eq!(response.len(), 0);
    }

    #[test]
    fn test_update_cw20_mapping() {
        let mut deps = setup(&["channel-3", "channel-7"], &[]);
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:foobar".to_string()),
        };
        let asset_info_second = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:foobar-second".to_string()),
        };

        let mut update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: "earth".to_string(),
            asset_info: asset_info.clone(),
            remote_decimals: 18,
            asset_info_decimals: 18,
        };

        // works with proper funds
        let mut msg = ExecuteMsg::UpdateMappingPair(update.clone());

        // unauthorized case
        let info = mock_info("foobar", &coins(1234567, "ucosm"));
        let res_err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
        assert_eq!(res_err, ContractError::Admin(AdminError::NotAdmin {}));

        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        // query to verify if the mapping has been updated
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappings {
                start_after: None,
                limit: None,
                order: None,
            },
        )
        .unwrap();
        let response: ListMappingResponse = from_binary(&mappings).unwrap();
        println!("response: {:?}", response);
        assert_eq!(
            response.pairs.first().unwrap().key,
            format!("{}/mars-channel/earth", CONTRACT_PORT)
        );

        // not found case
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappings {
                start_after: None,
                limit: None,
                order: None,
            },
        )
        .unwrap();
        let response: ListMappingResponse = from_binary(&mappings).unwrap();
        assert_ne!(response.pairs.first().unwrap().key, "foobar".to_string());

        // update existing key case must pass
        update.asset_info = asset_info_second.clone();
        msg = ExecuteMsg::UpdateMappingPair(update.clone());

        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // after update, cw20 denom now needs to be updated
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappings {
                start_after: None,
                limit: None,
                order: None,
            },
        )
        .unwrap();
        let response: ListMappingResponse = from_binary(&mappings).unwrap();
        println!("response: {:?}", response);
        assert_eq!(
            response.pairs.first().unwrap().key,
            format!("{}/mars-channel/earth", CONTRACT_PORT)
        );
        assert_eq!(
            response.pairs.first().unwrap().pair_mapping.asset_info,
            asset_info_second
        )
    }

    #[test]
    fn test_delete_cw20_mapping() {
        let mut deps = setup(&["channel-3", "channel-7"], &[]);
        let cw20_denom = AssetInfo::Token {
            contract_addr: Addr::unchecked("cw20:foobar".to_string()),
        };

        let update = UpdatePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: "earth".to_string(),
            asset_info: cw20_denom.clone(),
            remote_decimals: 18,
            asset_info_decimals: 18,
        };

        // works with proper funds
        let msg = ExecuteMsg::UpdateMappingPair(update.clone());

        let info = mock_info("gov", &coins(1234567, "ucosm"));
        execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

        // query to verify if the mapping has been updated
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappings {
                start_after: None,
                limit: None,
                order: None,
            },
        )
        .unwrap();
        let response: ListMappingResponse = from_binary(&mappings).unwrap();
        println!("response: {:?}", response);
        assert_eq!(
            response.pairs.first().unwrap().key,
            format!("{}/mars-channel/earth", CONTRACT_PORT)
        );

        // now try deleting
        let delete = DeletePairMsg {
            local_channel_id: "mars-channel".to_string(),
            denom: "earth".to_string(),
        };

        let mut msg = ExecuteMsg::DeleteMappingPair(delete.clone());

        // unauthorized delete case
        let info = mock_info("foobar", &coins(1234567, "ucosm"));
        let delete_err = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap_err();
        assert_eq!(delete_err, ContractError::Admin(AdminError::NotAdmin {}));

        let info = mock_info("gov", &coins(1234567, "ucosm"));

        // happy case
        msg = ExecuteMsg::DeleteMappingPair(delete.clone());
        execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

        // after update, the list cw20 mapping should be empty
        let mappings = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::PairMappings {
                start_after: None,
                limit: None,
                order: None,
            },
        )
        .unwrap();
        let response: ListMappingResponse = from_binary(&mappings).unwrap();
        println!("response: {:?}", response);
        assert_eq!(response.pairs.len(), 0)
    }

    // #[test]
    // fn proper_checks_on_execute_native() {
    //     let send_channel = "channel-5";
    //     let mut deps = setup(&[send_channel, "channel-10"], &[]);

    //     let mut transfer = TransferMsg {
    //         channel: send_channel.to_string(),
    //         remote_address: "foreign-address".to_string(),
    //         timeout: None,
    //         memo: Some("memo".to_string()),
    //     };

    //     // works with proper funds
    //     let msg = ExecuteMsg::Transfer(transfer.clone());
    //     let info = mock_info("foobar", &coins(1234567, "ucosm"));
    //     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    //     assert_eq!(res.messages[0].gas_limit, None);
    //     assert_eq!(1, res.messages.len());
    //     if let CosmosMsg::Ibc(IbcMsg::SendPacket {
    //         channel_id,
    //         data,
    //         timeout,
    //     }) = &res.messages[0].msg
    //     {
    //         let expected_timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
    //         assert_eq!(timeout, &expected_timeout.into());
    //         assert_eq!(channel_id.as_str(), send_channel);
    //         let msg: Ics20Packet = from_binary(data).unwrap();
    //         assert_eq!(msg.amount, Uint128::new(1234567));
    //         assert_eq!(msg.denom.as_str(), "ucosm");
    //         assert_eq!(msg.sender.as_str(), "foobar");
    //         assert_eq!(msg.receiver.as_str(), "foreign-address");
    //     } else {
    //         panic!("Unexpected return message: {:?}", res.messages[0]);
    //     }

    //     // reject with no funds
    //     let msg = ExecuteMsg::Transfer(transfer.clone());
    //     let info = mock_info("foobar", &[]);
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(err, ContractError::Payment(PaymentError::NoFunds {}));

    //     // reject with multiple tokens funds
    //     let msg = ExecuteMsg::Transfer(transfer.clone());
    //     let info = mock_info("foobar", &[coin(1234567, "ucosm"), coin(54321, "uatom")]);
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(err, ContractError::Payment(PaymentError::MultipleDenoms {}));

    //     // reject with bad channel id
    //     transfer.channel = "channel-45".to_string();
    //     let msg = ExecuteMsg::Transfer(transfer);
    //     let info = mock_info("foobar", &coins(1234567, "ucosm"));
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(
    //         err,
    //         ContractError::NoSuchChannel {
    //             id: "channel-45".to_string()
    //         }
    //     );
    // }

    // #[test]
    // fn proper_checks_on_execute_cw20() {
    //     let send_channel = "channel-15";
    //     let cw20_addr = "my-token";
    //     let mut deps = setup(&["channel-3", send_channel], &[(cw20_addr, 123456)]);

    //     let transfer = TransferMsg {
    //         channel: send_channel.to_string(),
    //         remote_address: "foreign-address".to_string(),
    //         timeout: Some(7777),
    //         memo: Some("memo".to_string()),
    //     };
    //     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
    //         sender: "my-account".into(),
    //         amount: Uint128::new(888777666),
    //         msg: to_binary(&transfer).unwrap(),
    //     });

    //     // works with proper funds
    //     let info = mock_info(cw20_addr, &[]);
    //     let res = execute(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    //     assert_eq!(1, res.messages.len());
    //     assert_eq!(res.messages[0].gas_limit, None);
    //     if let CosmosMsg::Ibc(IbcMsg::SendPacket {
    //         channel_id,
    //         data,
    //         timeout,
    //     }) = &res.messages[0].msg
    //     {
    //         let expected_timeout = mock_env().block.time.plus_seconds(7777);
    //         assert_eq!(timeout, &expected_timeout.into());
    //         assert_eq!(channel_id.as_str(), send_channel);
    //         let msg: Ics20Packet = from_binary(data).unwrap();
    //         assert_eq!(msg.amount, Uint128::new(888777666));
    //         assert_eq!(msg.denom, format!("cw20:{}", cw20_addr));
    //         assert_eq!(msg.sender.as_str(), "my-account");
    //         assert_eq!(msg.receiver.as_str(), "foreign-address");
    //     } else {
    //         panic!("Unexpected return message: {:?}", res.messages[0]);
    //     }

    //     // reject with tokens funds
    //     let info = mock_info("foobar", &coins(1234567, "ucosm"));
    //     let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    //     assert_eq!(err, ContractError::Payment(PaymentError::NonPayable {}));
    // }

    // #[test]
    // fn execute_cw20_fails_if_not_whitelisted_unless_default_gas_limit() {
    //     let send_channel = "channel-15";
    //     let mut deps = setup(&[send_channel], &[]);

    //     let cw20_addr = "my-token";
    //     let transfer = TransferMsg {
    //         channel: send_channel.to_string(),
    //         remote_address: "foreign-address".to_string(),
    //         timeout: Some(7777),
    //         memo: Some("memo".to_string()),
    //     };
    //     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
    //         sender: "my-account".into(),
    //         amount: Uint128::new(888777666),
    //         msg: to_binary(&transfer).unwrap(),
    //     });

    //     // rejected as not on allow list
    //     let info = mock_info(cw20_addr, &[]);
    //     let err = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap_err();
    //     assert_eq!(err, ContractError::NotOnAllowList);

    //     // add a default gas limit
    //     migrate(
    //         deps.as_mut(),
    //         mock_env(),
    //         MigrateMsg {
    //             default_gas_limit: Some(123456),
    //             fee_receiver: "receiver".to_string(),
    //             default_timeout: 100u64,
    //             fee_denom: "orai".to_string(),
    //             swap_router_contract: "foobar".to_string(),
    //         },
    //     )
    //     .unwrap();

    //     // try again
    //     execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    // }
    // test execute transfer back to native remote chain

    fn mock_receive_packet(
        remote_channel: &str,
        local_channel: &str,
        amount: u128,
        denom: &str,
        receiver: &str,
    ) -> IbcPacket {
        let data = Ics20Packet {
            // this is returning a foreign (our) token, thus denom is <port>/<channel>/<denom>
            denom: denom.to_string(),
            amount: amount.into(),
            sender: "remote-sender".to_string(),
            receiver: receiver.to_string(),
            memo: Some("memo".to_string()),
        };
        IbcPacket::new(
            to_binary(&data).unwrap(),
            IbcEndpoint {
                port_id: REMOTE_PORT.to_string(),
                channel_id: remote_channel.to_string(),
            },
            IbcEndpoint {
                port_id: CONTRACT_PORT.to_string(),
                channel_id: local_channel.to_string(),
            },
            3,
            Timestamp::from_seconds(1665321069).into(),
        )
    }

    #[test]
    fn proper_checks_on_execute_native_transfer_back_to_remote() {
        // arrange
        let remote_channel = "channel-5";
        let remote_address = "cosmos1603j3e4juddh7cuhfquxspl0p0nsun046us7n0";
        let custom_addr = "custom-addr";
        let original_sender = "original_sender";
        let denom = "uatom0x";
        let amount = 1234567u128;
        let token_addr = Addr::unchecked("token-addr".to_string());
        let asset_info = AssetInfo::Token {
            contract_addr: token_addr.clone(),
        };
        let cw20_raw_denom = token_addr.as_str();
        let local_channel = "channel-1234";
        let ratio = Ratio {
            nominator: 1,
            denominator: 10,
        };
        let fee_amount =
            Uint128::from(amount) * Decimal::from_ratio(ratio.nominator, ratio.denominator);
        let mut deps = setup(&[remote_channel, local_channel], &[]);
        TOKEN_FEE
            .save(deps.as_mut().storage, denom, &ratio)
            .unwrap();

        let pair = UpdatePairMsg {
            local_channel_id: local_channel.to_string(),
            denom: denom.to_string(),
            asset_info: asset_info.clone(),
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

        // execute
        let mut transfer = TransferBackMsg {
            local_channel_id: local_channel.to_string(),
            remote_address: remote_address.to_string(),
            remote_denom: denom.to_string(),
            timeout: Some(DEFAULT_TIMEOUT),
            memo: None,
        };

        // let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        //     sender: original_sender.to_string(),
        //     amount: Uint128::from(amount),
        //     msg: to_binary(&transfer).unwrap(),
        // });

        // // insufficient funds case because we need to receive from remote chain first
        let info = mock_info(cw20_raw_denom, &[]);
        // let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone());
        // println!("res: {:?}", res);
        // assert_eq!(
        //     res.unwrap_err(),
        //     ContractError::NoSuchChannelState {
        //         id: local_channel.to_string(),
        //         denom: get_key_ics20_ibc_denom("wasm.cosmos2contract", local_channel, denom)
        //     }
        // );

        // prepare some mock packets
        let recv_packet =
            mock_receive_packet(remote_channel, local_channel, amount, denom, custom_addr);

        // receive some tokens. Assume that the function works perfectly because the test case is elsewhere
        let ibc_msg = IbcPacketReceiveMsg::new(recv_packet.clone());
        ibc_packet_receive(deps.as_mut(), mock_env(), ibc_msg).unwrap();

        // error cases
        // revert transfer state to correct state
        transfer.local_channel_id = local_channel.to_string();
        let receive_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: original_sender.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&transfer).unwrap(),
        });

        // now we execute transfer back to remote chain
        let res = execute(deps.as_mut(), mock_env(), info.clone(), receive_msg).unwrap();

        assert_eq!(res.messages[0].gas_limit, None);
        println!("res messages: {:?}", res.messages);
        assert_eq!(2, res.messages.len()); // 2 because it also has deduct fee msg
        match res.messages[1].msg.clone() {
            CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id,
                data,
                timeout,
            }) => {
                let expected_timeout = mock_env().block.time.plus_seconds(DEFAULT_TIMEOUT);
                assert_eq!(timeout, expected_timeout.into());
                assert_eq!(channel_id.as_str(), local_channel);
                let msg: Ics20Packet = from_binary(&data).unwrap();
                assert_eq!(
                    msg.amount,
                    Uint128::new(1234567).sub(Uint128::from(fee_amount))
                );
                assert_eq!(
                    msg.denom.as_str(),
                    get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
                );
                assert_eq!(msg.sender.as_str(), original_sender);
                assert_eq!(msg.receiver.as_str(), remote_address);
                // assert_eq!(msg.memo, None);
            }
            _ => panic!("Unexpected return message: {:?}", res.messages[0]),
        }
        match res.messages[0].msg.clone() {
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr,
                msg,
                funds: _,
            }) => {
                assert_eq!(contract_addr, token_addr.to_string());
                assert_eq!(
                    msg,
                    to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "gov".to_string(),
                        amount: fee_amount.clone().checked_mul(Uint128::from(2u64)).unwrap() // mul with 2 because deduct when receive token then transfer back
                    })
                    .unwrap()
                );
            }
            _ => panic!("Unexpected return message: {:?}", res.messages[0]),
        }

        // since we reduce channel balance afterwards => need to manually reduce balance in test
        reduce_channel_balance(
            deps.as_mut().storage,
            local_channel.into(),
            &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom),
            Uint128::from(amount).checked_sub(fee_amount).unwrap(),
            false,
        )
        .unwrap();

        // check new channel state after reducing balance
        let chan = query_channel(deps.as_ref(), local_channel.into(), None).unwrap();
        assert_eq!(
            chan.balances,
            vec![Amount::native(
                fee_amount.u128(),
                &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
            )]
        );
        assert_eq!(
            chan.total_sent,
            vec![Amount::native(
                amount,
                &get_key_ics20_ibc_denom(CONTRACT_PORT, local_channel, denom)
            )]
        );

        // mapping pair error with wrong voucher denom
        let pair = UpdatePairMsg {
            local_channel_id: "not_registered_channel".to_string(),
            denom: denom.to_string(),
            asset_info: AssetInfo::Token {
                contract_addr: Addr::unchecked("random_cw20_denom".to_string()),
            },
            remote_decimals: 18u8,
            asset_info_decimals: 18u8,
        };

        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("gov", &[]),
            ExecuteMsg::UpdateMappingPair(pair),
        )
        .unwrap();

        transfer.local_channel_id = "not_registered_channel".to_string();
        let invalid_msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: original_sender.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&transfer).unwrap(),
        });
        let err = execute(deps.as_mut(), mock_env(), info.clone(), invalid_msg).unwrap_err();
        assert_eq!(err, ContractError::MappingPairNotFound {});
    }

    #[test]
    fn test_update_config() {
        // arrange
        let mut deps = setup(&[], &[]);
        let new_config = ExecuteMsg::UpdateConfig {
            admin: Some("helloworld".to_string()),
            default_timeout: Some(1),
            default_gas_limit: None,
            fee_denom: Some("hehe".to_string()),
            swap_router_contract: Some("new_router".to_string()),
            token_fee: Some(vec![
                TokenFee {
                    token_denom: "orai".to_string(),
                    ratio: Ratio {
                        nominator: 1,
                        denominator: 10,
                    },
                },
                TokenFee {
                    token_denom: "atom".to_string(),
                    ratio: Ratio {
                        nominator: 1,
                        denominator: 5,
                    },
                },
            ]),
            relayer_fee: Some(vec![RelayerFee {
                prefix: "foo".to_string(),
                fee: Uint128::from(1000000u64),
            }]),
            fee_receiver: None,
        };
        // unauthorized case
        let unauthorized_info = mock_info(&String::from("somebody"), &[]);
        let is_err = execute(
            deps.as_mut(),
            mock_env(),
            unauthorized_info,
            new_config.clone(),
        )
        .is_err();
        assert_eq!(is_err, true);
        // valid case
        let info = mock_info(&String::from("gov"), &[]);
        execute(deps.as_mut(), mock_env(), info, new_config).unwrap();
        let config: ConfigResponse =
            from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
        assert_eq!(config.default_gas_limit, None);
        assert_eq!(config.default_timeout, 1);
        assert_eq!(config.fee_denom, "hehe".to_string());
        assert_eq!(config.swap_router_contract, "new_router".to_string());
        assert_eq!(
            TOKEN_FEE
                .range(deps.as_ref().storage, None, None, Order::Ascending)
                .count(),
            2usize
        );
        assert_eq!(
            TOKEN_FEE
                .load(deps.as_ref().storage, "orai")
                .unwrap()
                .denominator,
            10
        );
        assert_eq!(
            TOKEN_FEE
                .load(deps.as_ref().storage, "atom")
                .unwrap()
                .denominator,
            5
        );
        assert_eq!(
            RELAYER_FEE.load(deps.as_ref().storage, "foo").unwrap(),
            Uint128::from(1000000u64),
        );
    }

    #[test]
    fn test_asset_info() {
        let asset_info = AssetInfo::NativeToken {
            denom: "orai".to_string(),
        };
        assert_eq!(asset_info.to_string(), "orai".to_string());
        let asset_info = AssetInfo::Token {
            contract_addr: Addr::unchecked("oraiaxbc".to_string()),
        };
        assert_eq!(asset_info.to_string(), "oraiaxbc".to_string())
    }
}
