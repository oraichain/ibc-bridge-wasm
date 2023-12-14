use cosmwasm_std::{Api, QuerierWrapper, StdError, StdResult};
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

pub fn denom_to_asset_info(
    querier: &QuerierWrapper,
    api: &dyn Api,
    denom: &str,
) -> StdResult<AssetInfo> {
    let info = if querier
        .query_wasm_smart::<TokenInfoResponse>(denom, &Cw20QueryMsg::TokenInfo {})
        .is_ok()
    {
        AssetInfo::Token {
            contract_addr: api.addr_validate(denom)?,
        }
    } else {
        AssetInfo::NativeToken {
            denom: denom.to_string(),
        }
    };
    Ok(info)
}

pub fn to_orai_bridge_address(address: &str) -> StdResult<String> {
    let decode_result = bech32::decode(address).unwrap();
    let oraib_address = bech32::encode("oraib", decode_result.1, bech32::Variant::Bech32).unwrap();

    Ok(oraib_address)
}

#[test]
fn test_get_prefix_decode_bech32() {
    let result = get_prefix_decode_bech32("cosmos1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejl67nlm").unwrap();
    assert_eq!(result, "cosmos".to_string());
}

#[test]
fn test_to_orai_bridge_address() {
    let result = to_orai_bridge_address("orai1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejvfgs7g").unwrap();
    assert_eq!(
        result,
        "oraib1g4h64yjt0fvzv5v2j8tyfnpe5kmnetejmgvu0t".to_string()
    );
}
