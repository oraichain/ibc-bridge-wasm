use cosmwasm_std::{Binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError, Uint128};

use cw20_ics20_msg::{
    amount::Amount,
    helper::{denom_to_asset_info, parse_asset_info_denom},
    ibc_hooks::{HookMethods, IbcHooksUniversalSwap},
    receiver::DestinationInfo,
};
use cw_utils::one_coin;
use oraiswap::asset::{Asset, AssetInfo};

use crate::{
    ibc::{
        find_evm_pair_mapping, get_follow_up_msgs, parse_ibc_channel_without_sanity_checks,
        parse_ibc_denom_without_sanity_checks, process_deduct_fee,
    },
    msg::PairQuery,
    query_helper::get_mappings_from_asset_info,
    state::{CONFIG, CONVERTER_INFO},
    ContractError,
};

pub fn ibc_hooks_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    func: HookMethods,
    args: Binary,
) -> Result<Response, ContractError> {
    match func {
        HookMethods::UniversalSwap => ibc_hooks_universal_swap(deps, env, info, args),
    }
}

pub fn ibc_hooks_universal_swap(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    args: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // check exactly one coin was sent
    let source_coin = one_coin(&info)?;

    let hooks_info = IbcHooksUniversalSwap::from_binary(deps.api, &args)?;
    let destination = DestinationInfo {
        receiver: hooks_info.destination_receiver,
        destination_channel: hooks_info.destination_channel,
        destination_denom: hooks_info.destination_denom,
    };

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut token_fee = Uint128::zero();
    let mut relayer_fee = Uint128::zero();

    let mut to_send = Asset {
        amount: source_coin.amount.clone(),
        info: AssetInfo::NativeToken {
            denom: source_coin.denom.clone(),
        },
    };

    // if this receive token is already registered which needs to be converted, then execute the converter before
    let converter_info = CONVERTER_INFO.may_load(deps.storage, &to_send.info.to_vec(deps.api)?)?;
    if let Some(converter_info) = converter_info {
        if converter_info.from.eq(&to_send.info) {
            let from_asset = Asset {
                info: AssetInfo::NativeToken {
                    denom: source_coin.denom.clone(),
                },
                amount: source_coin.amount,
            };
            let (msg, return_amount) = config.converter_contract.process_convert(
                &deps.querier,
                &from_asset,
                &converter_info,
            )?;
            msgs.push(msg);
            to_send = return_amount;
        }
    }

    let destination_asset_info_on_orai =
        denom_to_asset_info(deps.api, &destination.destination_denom);
    let mut destination_pair_mapping: Option<PairQuery> = None;
    let mut to_send_amount =
        Amount::from_parts(parse_asset_info_denom(to_send.info.clone()), to_send.amount);
    let follow_up_msg_data;

    // There are 2 cases:
    // 1. If the destination chain is Oraichain
    // 2. If the destination chain is another

    // case 1: destination chain is Orai (destination channel is empty)
    if destination.destination_channel.is_empty() {
        follow_up_msg_data = get_follow_up_msgs(
            deps.storage,
            deps.api,
            &deps.querier,
            env,
            to_send_amount,
            to_send.info,
            destination_asset_info_on_orai,
            "",
            &destination.receiver,
            &destination,
            "",
            destination_pair_mapping,
        )?;
    } else {
        // case 2: the destination chain is another

        // requires destination_channel and destination_denom not to be empty
        if destination.destination_channel.is_empty() || destination.destination_denom.is_empty() {
            return Err(ContractError::Std(StdError::generic_err(
                "Require destination denom & channel in memo",
            )));
        }
        let pair_mappings =
            get_mappings_from_asset_info(deps.storage, destination_asset_info_on_orai.to_owned())?;

        let is_cosmos_based = destination.is_receiver_cosmos_based();
        if is_cosmos_based {
            destination_pair_mapping = pair_mappings.clone().into_iter().find(|pair_query| {
                parse_ibc_channel_without_sanity_checks(&pair_query.key)
                    .unwrap_or_default()
                    .eq(&destination.destination_channel)
            });
        } else {
            let (is_evm_based, evm_prefix) = destination.is_receiver_evm_based();
            if is_evm_based {
                destination_pair_mapping = pair_mappings.into_iter().find(|pair_query| {
                    find_evm_pair_mapping(
                        &pair_query.key,
                        &evm_prefix,
                        &destination.destination_channel,
                    )
                });
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "Destination chain is invalid!",
                )));
            }
        }

        // calc fee
        if let Some(mapping) = destination_pair_mapping.clone() {
            let remote_destination_denom =
                parse_ibc_denom_without_sanity_checks(&mapping.key)?.to_string();

            let fee_data = process_deduct_fee(
                deps.storage,
                &deps.querier,
                deps.api,
                &destination.receiver.clone(),
                &remote_destination_denom,
                to_send_amount.clone(),
                mapping.pair_mapping.asset_info_decimals,
                &config.swap_router_contract,
            )?;

            // if the fees have consumed all user funds, we send all the fees to our token fee receiver
            if fee_data.deducted_amount.is_zero() {
                return Ok(Response::new()
                    .add_message(
                        to_send_amount.send_amount(config.token_fee_receiver.into_string(), None),
                    )
                    .add_attributes(vec![
                        ("action", "receive_ibc_hooks"),
                        ("receiver", &destination.receiver),
                        ("denom", source_coin.denom.as_str()),
                        ("amount", &source_coin.amount.to_string()),
                        ("destination", &destination.receiver),
                        ("token_fee", &fee_data.token_fee.amount().to_string()),
                        ("relayer_fee", &fee_data.relayer_fee.amount().to_string()),
                    ]));
            }

            if !fee_data.token_fee.is_empty() {
                msgs.push(
                    fee_data
                        .token_fee
                        .send_amount(config.token_fee_receiver.into_string(), None),
                );
                token_fee = fee_data.token_fee.amount();
            }
            if !fee_data.relayer_fee.is_empty() {
                msgs.push(
                    fee_data
                        .relayer_fee
                        .send_amount(config.relayer_fee_receiver.to_string(), None),
                );
                relayer_fee = fee_data.relayer_fee.amount();
            }

            to_send_amount = Amount::from_parts(
                parse_asset_info_denom(to_send.info.clone()),
                fee_data.deducted_amount,
            );
        }

        follow_up_msg_data = get_follow_up_msgs(
            deps.storage,
            deps.api,
            &deps.querier,
            env.clone(),
            to_send_amount,
            to_send.info,
            destination_asset_info_on_orai,
            &hooks_info.bridge_receiver,
            &hooks_info.receiver,
            &destination,
            &destination.destination_channel,
            destination_pair_mapping,
        )?;
    }

    // check follow up msg will be success
    if !follow_up_msg_data.follow_up_msg.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "ibc_error_msg: {}",
            follow_up_msg_data.follow_up_msg,
        ))));
    }
    msgs.extend(
        follow_up_msg_data
            .sub_msgs
            .into_iter()
            .map(|sub_msg| sub_msg.msg),
    );

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "receive_ibc_hooks"),
            ("receiver", &destination.receiver),
            ("denom", source_coin.denom.as_str()),
            ("amount", &source_coin.amount.to_string()),
            ("token_fee", &token_fee.to_string()),
            ("relayer_fee", &relayer_fee.to_string()),
        ])
        .add_messages(msgs))
}
