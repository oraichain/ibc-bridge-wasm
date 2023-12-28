use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Coin, CosmosMsg, QuerierWrapper, StdResult, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use oraiswap::{
    asset::{Asset, AssetInfo},
    converter::{self, ConvertInfoResponse},
    math::Converter128,
};

pub enum ConvertType {
    FromSource,
    ToSource,
}

#[cw_serde]
pub struct ConverterController(pub String);

impl ConverterController {
    pub fn addr(&self) -> String {
        self.0.clone()
    }

    pub fn converter_info(
        &self,
        querier: &QuerierWrapper,
        source_info: &AssetInfo,
    ) -> Option<ConvertInfoResponse> {
        match querier.query_wasm_smart(
            self.addr(),
            &converter::QueryMsg::ConvertInfo {
                asset_info: source_info.to_owned(),
            },
        ) {
            Ok(val) => return val,
            Err(_) => return None,
        };
    }

    pub fn process_convert(
        &self,
        querier: &QuerierWrapper,
        source_info: &AssetInfo,
        amount: Uint128,
        convert_type: ConvertType,
    ) -> StdResult<(Option<CosmosMsg>, Asset)> {
        match self.converter_info(querier, source_info) {
            None => {
                return Ok((
                    None,
                    Asset {
                        info: source_info.to_owned(),
                        amount,
                    },
                ))
            }
            Some(converter_info) => match convert_type {
                ConvertType::FromSource => {
                    let return_asset = Asset {
                        info: converter_info.token_ratio.info.clone(),
                        amount: amount * converter_info.token_ratio.ratio,
                    };

                    let msg = match source_info.to_owned() {
                        AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: self.addr(),
                            msg: to_binary(&converter::ExecuteMsg::Convert {})?,
                            funds: vec![Coin { denom, amount }],
                        }),
                        AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: contract_addr.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Send {
                                contract: self.addr(),
                                amount,
                                msg: to_binary(&converter::Cw20HookMsg::Convert {})?,
                            })?,
                            funds: vec![],
                        }),
                    };

                    return Ok((Some(msg), return_asset));
                }
                ConvertType::ToSource => {
                    let return_asset = Asset {
                        info: source_info.to_owned(),
                        amount: amount.checked_div_decimal(converter_info.token_ratio.ratio)?,
                    };

                    let msg = match converter_info.token_ratio.info {
                        AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: self.addr(),
                            msg: to_binary(&converter::ExecuteMsg::ConvertReverse {
                                from_asset: source_info.to_owned(),
                            })?,
                            funds: vec![Coin { denom, amount }],
                        }),
                        AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: contract_addr.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Send {
                                contract: self.addr(),
                                amount,
                                msg: to_binary(&converter::Cw20HookMsg::ConvertReverse {
                                    from: source_info.to_owned(),
                                })?,
                            })?,
                            funds: vec![],
                        }),
                    };

                    return Ok((Some(msg), return_asset));
                }
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_dependencies, Addr, Api, Decimal, Uint128};
    use cw_storage_plus::KeyDeserialize;
    use oraiswap::math::Converter128;

    #[test]
    fn test_convert_from_string_to_addr_from_slice() {
        let addr_string = "cosmos19a4cjjdlx5fpsgfz7t4tgh6kn6heqg874ylr2y";
        let deps = mock_dependencies();
        let addr_from_api = deps.api.addr_validate(addr_string).unwrap();
        let addr = Addr::from_slice(addr_string.as_bytes()).unwrap();
        assert_eq!(addr_from_api, addr);
    }

    #[test]
    fn test_div_uint128_with_decimals_atomic() {
        let amount = Uint128::from(100u64);
        let decimal = Decimal::from_ratio(1u64, 20u64);
        let div_ceil_result = amount.div_ceil(decimal);
        let check_div_result = amount.checked_div_decimal(decimal).unwrap();
        assert_eq!(div_ceil_result, check_div_result)
    }
}
