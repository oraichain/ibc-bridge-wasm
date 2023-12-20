use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Coin, CosmosMsg, QuerierWrapper, StdError, StdResult, WasmMsg};
use cw20::Cw20ExecuteMsg;
use oraiswap::{
    asset::{Asset, AssetInfo},
    converter::{self, ConvertInfoResponse},
};

#[cw_serde]
pub struct ConverterInfo {
    pub from: AssetInfo,
    pub to: AssetInfo,
}

#[cw_serde]
pub struct ConverterController(pub String);

impl ConverterController {
    pub fn addr(&self) -> String {
        self.0.clone()
    }

    pub fn process_convert(
        &self,
        querier: &QuerierWrapper,
        from_asset: &Asset,
        converter_info: &ConverterInfo,
    ) -> StdResult<(CosmosMsg, Asset)> {
        let info: ConvertInfoResponse = querier.query_wasm_smart(
            self.addr(),
            &converter::QueryMsg::ConvertInfo {
                asset_info: converter_info.from.clone(),
            },
        )?;

        if converter_info.to.ne(&info.token_ratio.info) {
            return Err(StdError::generic_err(
                "Convert error. To token does not match converter info",
            ));
        }

        if converter_info.from.eq(&from_asset.info) {
            let return_asset = Asset {
                info: info.token_ratio.info.clone(),
                amount: from_asset.amount * info.token_ratio.ratio,
            };

            let msg = match from_asset.info.clone() {
                AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: self.addr(),
                    msg: to_binary(&converter::ExecuteMsg::Convert {})?,
                    funds: vec![Coin {
                        denom,
                        amount: from_asset.amount,
                    }],
                }),
                AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: self.addr(),
                        amount: from_asset.amount,
                        msg: to_binary(&converter::Cw20HookMsg::Convert {})?,
                    })?,
                    funds: vec![],
                }),
            };

            return Ok((msg, return_asset));
        } else if converter_info.to.eq(&from_asset.info) {
            let return_asset = Asset {
                info: converter_info.from.clone(),
                amount: from_asset.amount.div_ceil(info.token_ratio.ratio),
            };

            let msg = match from_asset.info.clone() {
                AssetInfo::NativeToken { denom } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: self.addr(),
                    msg: to_binary(&converter::ExecuteMsg::ConvertReverse {
                        from_asset: converter_info.from.clone(),
                    })?,
                    funds: vec![Coin {
                        denom,
                        amount: from_asset.amount,
                    }],
                }),
                AssetInfo::Token { contract_addr } => CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: self.addr(),
                        amount: from_asset.amount,
                        msg: to_binary(&converter::Cw20HookMsg::ConvertReverse {
                            from: converter_info.from.clone(),
                        })?,
                    })?,
                    funds: vec![],
                }),
            };

            return Ok((msg, return_asset));
        }

        return Err(StdError::generic_err("Convert error"));
    }
}
