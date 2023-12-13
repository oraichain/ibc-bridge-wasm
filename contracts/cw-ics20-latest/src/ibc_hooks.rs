use cosmwasm_std::{
    attr, Attribute, Binary, CosmosMsg, DepsMut, Env, MessageInfo, Order, Response, StdError,
    StdResult,
};

use cw20_ics20_msg::{
    amount::Amount,
    helper::{denom_to_asset_info, parse_asset_info_denom},
    receiver::{BridgeInfo, DestinationInfo},
};
use cw_utils::one_coin;
use oraiswap::asset::{Asset, AssetInfo};

use crate::{
    ibc::{
        find_evm_pair_mapping, get_follow_up_msgs, parse_ibc_channel_without_sanity_checks,
        process_deduct_fee,
    },
    state::{ics20_denoms, MappingMetadata, CONFIG, CONVERTER_INFO},
    ContractError,
};

pub fn ibc_hooks_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    args: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attrs: Vec<Attribute> = vec![attr("action", "receive_ibc_hooks")];

    let mut destination_memo = String::default();
    let mut bridge_info_memo = String::default();

    // check exactly one coin was sent
    let source_coin = one_coin(&info)?;
    let mut to_send = Asset {
        amount: source_coin.amount.clone(),
        info: AssetInfo::NativeToken {
            denom: source_coin.denom.clone(),
        },
    };

    // unmarshal args
    let mut index = 0;
    while index < args.len() {
        match args[index] {
            // 0 : ConvertToken
            0 => {
                index += 1;
                let from_asset = Asset {
                    info: AssetInfo::NativeToken {
                        denom: source_coin.denom.clone(),
                    },
                    amount: source_coin.amount,
                };
                let converter_info = match CONVERTER_INFO
                    .may_load(deps.storage, &from_asset.info.to_vec(deps.api)?)?
                {
                    Some(info) => info,
                    None => {
                        return Err(ContractError::Std(StdError::generic_err(
                            "Converter_info not found",
                        )))
                    }
                };

                let (msg, return_amount) = config.converter_contract.process_convert(
                    deps.as_ref(),
                    &from_asset,
                    &converter_info,
                )?;

                msgs.push(msg);
                // to_send was
                to_send = return_amount;
            }
            // 1: Destination Info
            1 => {
                index += 1;
                let value_length = args[index] as usize;
                destination_memo =
                    String::from_utf8((&args[index..index + value_length]).to_vec())?;
                index += value_length;
            }
            // 2: Orai Gravity Bridge Info
            2 => {
                index += 1;
                let value_length = args[index] as usize;
                bridge_info_memo =
                    String::from_utf8((&args[index..index + value_length]).to_vec())?;
                index += value_length;
            }
            _ => return Err(ContractError::InvalidIbcHooksMethods),
        }
    }

    let destination = DestinationInfo::from_str(&destination_memo);
    let destination_asset_info_on_orai =
        denom_to_asset_info(&deps.querier, deps.api, &destination.destination_denom)?;
    let remote_destination_denom: String = "".to_string();
    let mut destination_pair_mapping: Option<(String, MappingMetadata)> = None;

    // there are 2 cases:
    // 1. Destination chain is Orai
    // 2. Destination chain is others
    // always require destination.receiver
    if destination.receiver.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(
            "Require destination receiver in memo",
        )));
    }

    let to_send_amount =
        Amount::from_parts(parse_asset_info_denom(to_send.info.clone()), to_send.amount);

    let follow_up_msg_data;

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
        // case 2
        // Bridge Info is require
        // if destination is evm, it will use to create IBC Packet send from Orai --> Orai Bridge
        // if destination is cosmos, it will use to handle refund
        if bridge_info_memo.is_empty() {
            return Err(ContractError::Std(StdError::generic_err(
                "Require OraiBridgeInfo receiver in memo",
            )));
        }
        let bridge_info = BridgeInfo::from_str(&bridge_info_memo)?;

        let pair_mappings: Vec<(String, MappingMetadata)> = ics20_denoms()
            .idx
            .asset_info
            .prefix(destination_asset_info_on_orai.to_string())
            .range(deps.storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<(String, MappingMetadata)>>>()?;

        let mut remote_address = "".to_string();
        let is_cosmos_based = destination.is_receiver_cosmos_based();
        if is_cosmos_based {
            destination_pair_mapping = pair_mappings.clone().into_iter().find(|(key, _)| {
                parse_ibc_channel_without_sanity_checks(key)
                    .unwrap_or_default()
                    .eq(&destination.destination_channel)
            });
            remote_address = destination.receiver.clone();
        } else {
            let (is_evm_based, evm_prefix) = destination.is_receiver_evm_based();
            if is_evm_based {
                destination_pair_mapping = pair_mappings.into_iter().find(|(key, _)| {
                    find_evm_pair_mapping(&key, &evm_prefix, &destination.destination_channel)
                });
            } else {
                return Err(ContractError::Std(StdError::generic_err(
                    "Destination chain is invalid!",
                )));
            }
        }

        // calc fee
        let fee_data = process_deduct_fee(
            deps.storage,
            &deps.querier,
            deps.api,
            &remote_address,
            &remote_destination_denom,
            to_send_amount.clone(),
            destination_pair_mapping
                .clone()
                .unwrap()
                .1
                .asset_info_decimals,
            &config.swap_router_contract,
        )?;

        // if the fees have consumed all user funds, we send all the fees to our token fee receiver
        if fee_data.deducted_amount.is_zero() {
            return Ok(Response::new()
                .add_message(
                    to_send_amount.send_amount(config.token_fee_receiver.into_string(), None),
                )
                .add_attributes(vec![
                    ("action", "receive_ibc_hookds"),
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
            )
        }
        if !fee_data.relayer_fee.is_empty() {
            msgs.push(
                fee_data
                    .relayer_fee
                    .send_amount(config.relayer_fee_receiver.to_string(), None),
            )
        }

        let new_deducted_to_send_amount = Amount::from_parts(
            parse_asset_info_denom(to_send.info.clone()),
            fee_data.deducted_amount,
        );
        follow_up_msg_data = get_follow_up_msgs(
            deps.storage,
            deps.api,
            &deps.querier,
            env.clone(),
            new_deducted_to_send_amount,
            to_send.info,
            destination_asset_info_on_orai,
            &bridge_info.receiver,
            &bridge_info.sender,
            &destination,
            &bridge_info.channel,
            destination_pair_mapping,
        )?;
        attrs.push(attr("orai_sender", bridge_info.sender));
        attrs.push(attr("receiver", bridge_info.receiver));
        attrs.push(attr("amount", fee_data.deducted_amount.to_string()));
    }

    // Different from ibc-wasm, we don't handle error of follow up msg.
    // If an error occurred => ibc-hooks messages will revert => packet receive revert at app chain level
    if !follow_up_msg_data.follow_up_msg.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "ibc_error_msg: {}",
            follow_up_msg_data.follow_up_msg
        ))));
    }

    msgs.extend(
        follow_up_msg_data
            .sub_msgs
            .into_iter()
            .map(|sub_msg| sub_msg.msg),
    );

    Ok(Response::new().add_attributes(attrs).add_messages(msgs))
}

// #[test]
// fn test_decode_ibc_hooks_receive() {}
