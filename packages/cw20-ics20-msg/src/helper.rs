use cosmwasm_std::{Addr, QuerierWrapper, StdError, StdResult};
use cw20::{Cw20QueryMsg, TokenInfoResponse};
use oraiswap::asset::AssetInfo;

pub fn get_prefix_decode_bech32(address: &str) -> StdResult<String> {
    let decode_result = bech32::decode(address);
    if decode_result.is_err() {
        return Err(StdError::generic_err(format!(
            "Cannot decode remote sender: {}",
            address
        )));
    }
    Ok(decode_result.unwrap().0)
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

pub fn denom_to_asset_info(querier: &QuerierWrapper, denom: &str) -> AssetInfo {
    if querier
        .query_wasm_smart::<TokenInfoResponse>(denom.clone(), &Cw20QueryMsg::TokenInfo {})
        .is_ok()
    {
        return AssetInfo::Token {
            contract_addr: Addr::unchecked(denom),
        };
    }
    AssetInfo::NativeToken {
        denom: denom.to_string(),
    }
}
