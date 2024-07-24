use cosmwasm_std::{to_json_binary, Binary, CosmosMsg, DepsMut, Env, MessageInfo, Response};

use cw20_ics20_msg::{
    amount::Amount, converter::ConvertType, helper::parse_asset_info_denom, ibc_hooks::HookMethods,
};
use cw_utils::one_coin;
use oraiswap::asset::AssetInfo;

use crate::{state::CONFIG, ContractError};
use skip::entry_point::ExecuteMsg as EntryPointExecuteMsg;

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
    _env: Env,
    info: MessageInfo,
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

    let swap_then_post_action_msg =
        Amount::from_parts(parse_asset_info_denom(&to_send.info), to_send.amount).send_amount(
            config.osor_entrypoint_contract,
            Some(to_json_binary(&EntryPointExecuteMsg::UniversalSwap {
                memo: args.to_base64(),
            })?),
        );
    msgs.push(swap_then_post_action_msg);

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "receive_ibc_hooks"),
            ("denom", source_coin.denom.as_str()),
            ("amount", &source_coin.amount.to_string()),
        ])
        .add_messages(msgs))
}
