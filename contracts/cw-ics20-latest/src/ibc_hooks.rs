use cosmwasm_std::{Binary, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use cw20_ics20_msg::{
    amount::Amount, converter::ConvertType, helper::parse_asset_info_denom, ibc_hooks::HookMethods,
};
use cw_utils::one_coin;
use oraiswap::asset::AssetInfo;

use crate::{ibc::get_follow_up_msgs, state::CONFIG, ContractError};

pub fn ibc_hooks_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    func: HookMethods,
    orai_receiver: String,
    args: Binary,
) -> Result<Response, ContractError> {
    match func {
        HookMethods::UniversalSwap => {
            ibc_hooks_universal_swap(deps, env, info, orai_receiver, args)
        }
    }
}

pub fn ibc_hooks_universal_swap(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    orai_receiver: String,
    args: Binary,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // check exactly one coin was sent
    let source_coin = one_coin(&info)?;

    let mut msgs: Vec<CosmosMsg> = vec![];

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
    let sub_msgs = get_follow_up_msgs(
        deps.storage,
        orai_receiver,
        Amount::from_parts(parse_asset_info_denom(&to_send.info), to_send.amount),
        Some(args.to_base64()),
    )?;

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "receive_ibc_hooks"),
            ("denom", source_coin.denom.as_str()),
            ("amount", &source_coin.amount.to_string()),
        ])
        .add_messages(msgs)
        .add_submessages(sub_msgs))
}
