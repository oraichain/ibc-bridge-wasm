use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Coin, CosmosMsg, QuerierWrapper, StdResult, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use oraiswap::{
    asset::{Asset, AssetInfo},
    converter::{self, ConvertInfoResponse},
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
        let res = match querier.query_wasm_smart(
            self.addr(),
            &converter::QueryMsg::ConvertInfo {
                asset_info: source_info.to_owned(),
            },
        ) {
            Ok(val) => val,
            Err(_) => {
                return Ok((
                    None,
                    Asset {
                        info: source_info.to_owned(),
                        amount,
                    },
                ))
            }
        };

        let converter_info: ConvertInfoResponse = res;

        match convert_type {
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
                    amount: amount.div_ceil(converter_info.token_ratio.ratio),
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
        }
    }
}
