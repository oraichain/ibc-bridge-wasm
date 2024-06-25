use cosmwasm_std::{Binary, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdError, Uint128};

use cw20_ics20_msg::{
    amount::Amount,
    converter::ConvertType,
    helper::parse_asset_info_denom,
    ibc_hooks::{HookMethods, IbcHooksUniversalSwap},
    receiver::DestinationInfo,
};
use cw_utils::one_coin;
use oraiswap::asset::AssetInfo;

use crate::{
    ibc::{get_follow_up_msgs, process_deduct_fee},
    query_helper::get_destination_info_on_orai,
    state::CONFIG,
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
    let token_fee;
    let relayer_fee;

    let (msg, to_send) = config.converter_contract.process_convert(
        &deps.querier,
        &AssetInfo::NativeToken {
            denom: source_coin.denom.clone(),
        },
        source_coin.amount,
        ConvertType::FromSource,
    )?;

    if let Some(msg) = msg {
        msgs.push(msg);
    }

    // if destination denom is empty, set destination denom to ibc denom receive
    let (destination_asset_info_on_orai, destination_pair_mapping) =
        if destination.destination_denom.is_empty() {
            (to_send.info.clone(), None)
        } else {
            get_destination_info_on_orai(
                deps.storage,
                deps.api,
                &env,
                &destination.destination_channel,
                &destination.destination_denom,
            )
        };

    let mut to_send_amount =
        Amount::from_parts(parse_asset_info_denom(&to_send.info), to_send.amount);
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
            destination_pair_mapping,
        )?;
        token_fee = Uint128::zero();
        relayer_fee = Uint128::zero();
    } else {
        // case 2: the destination chain is another

        // requires both destination_channel and destination_denom not to be empty
        if destination.destination_denom.is_empty() {
            return Err(ContractError::Std(StdError::generic_err(
                "Require destination denom & channel in memo",
            )));
        }

        // calc fee
        if destination_pair_mapping.is_some() {
            let fee_data = process_deduct_fee(
                deps.storage,
                &deps.querier,
                deps.api,
                &destination.receiver.clone(),
                &destination.destination_denom,
                to_send_amount.clone(),
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

            token_fee = fee_data.token_fee.amount();
            if !token_fee.is_zero() {
                msgs.push(
                    fee_data
                        .token_fee
                        .send_amount(config.token_fee_receiver.into_string(), None),
                );
            }
            relayer_fee = fee_data.relayer_fee.amount();
            if !relayer_fee.is_zero() {
                msgs.push(
                    fee_data
                        .relayer_fee
                        .send_amount(config.relayer_fee_receiver.to_string(), None),
                );
            }

            to_send_amount = Amount::from_parts(
                parse_asset_info_denom(&to_send.info),
                fee_data.deducted_amount,
            );
        } else {
            token_fee = Uint128::zero();
            relayer_fee = Uint128::zero();
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
            destination_pair_mapping,
        )?;
    }

    // check follow up msg will be success
    if !follow_up_msg_data.is_success {
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
