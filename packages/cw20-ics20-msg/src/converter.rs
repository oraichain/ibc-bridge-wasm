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
    use std::marker::PhantomData;

    use cosmwasm_std::{
        from_binary, from_slice,
        testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
        to_binary, Addr, Api, Coin, ContractResult, CosmosMsg, Decimal, Empty, OwnedDeps, Querier,
        QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmMsg, WasmQuery,
    };
    use cw20::Cw20ExecuteMsg;
    use cw_storage_plus::KeyDeserialize;
    use oraiswap::{
        asset::{Asset, AssetInfo},
        converter::{self, ConvertInfoResponse, TokenRatio},
        math::Converter128,
    };

    use super::{ConvertType, ConverterController};

    /// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
    /// this uses our CustomQuerier.
    pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
        let custom_querier: WasmMockQuerier =
            WasmMockQuerier::new(MockQuerier::new(&[(&MOCK_CONTRACT_ADDR, &vec![])]));

        OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: custom_querier,
            custom_query_type: PhantomData::default(),
        }
    }

    pub struct WasmMockQuerier {
        base: MockQuerier,
    }

    impl WasmMockQuerier {
        pub fn new(base: MockQuerier<Empty>) -> Self {
            WasmMockQuerier { base }
        }
    }

    impl Querier for WasmMockQuerier {
        fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
            // MockQuerier doesn't support Custom, so we ignore it completely here
            let request: QueryRequest<Empty> = match from_slice(bin_request) {
                Ok(v) => v,
                Err(e) => {
                    return SystemResult::Err(SystemError::InvalidRequest {
                        error: format!("Parsing query request: {}", e),
                        request: bin_request.into(),
                    })
                }
            };
            self.handle_query(&request)
        }
    }
    impl WasmMockQuerier {
        pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
            match &request {
                QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: _,
                    msg,
                }) => match from_binary(msg) {
                    Ok(converter::QueryMsg::ConvertInfo { asset_info }) => {
                        if asset_info.eq(&AssetInfo::NativeToken {
                            denom: "inj".to_string(),
                        }) {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&ConvertInfoResponse {
                                    token_ratio: TokenRatio {
                                        info: AssetInfo::Token {
                                            contract_addr: Addr::unchecked("orai123"),
                                        },
                                        ratio: Decimal::from_ratio(1u128, 100u128),
                                    },
                                })
                                .unwrap(),
                            ))
                        } else {
                            SystemResult::Err(SystemError::InvalidRequest {
                                error: "Converter info not found".to_string(),
                                request: msg.as_slice().into(),
                            })
                        }
                    }
                    _ => panic!("DO NOT ENTER HERE"),
                },
                _ => self.base.handle_query(request),
            }
        }
    }

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

    #[test]
    fn test_query_converter_info() {
        // query converter info
        let deps = mock_dependencies();
        let converter_contract = ConverterController("converter".to_string());

        // case query failed, this token has not registered
        let res = converter_contract.converter_info(
            &deps.as_ref().querier,
            &AssetInfo::NativeToken {
                denom: "cosmos".to_string(),
            },
        );
        assert_eq!(res, None);

        // case success
        let res = converter_contract.converter_info(
            &deps.as_ref().querier,
            &AssetInfo::NativeToken {
                denom: "inj".to_string(),
            },
        );
        assert_eq!(
            res,
            Some(ConvertInfoResponse {
                token_ratio: TokenRatio {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("orai123"),
                    },
                    ratio: Decimal::from_ratio(1u128, 100u128),
                },
            })
        )
    }

    #[test]
    fn test_process_convert() {
        let deps = mock_dependencies();
        let converter_contract = ConverterController("converter".to_string());

        // case: token not registered
        let res = converter_contract
            .process_convert(
                &deps.as_ref().querier,
                &AssetInfo::NativeToken {
                    denom: "cosmos".to_string(),
                },
                Uint128::from(100000u128),
                ConvertType::FromSource,
            )
            .unwrap();
        assert_eq!(
            res,
            (
                None,
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "cosmos".to_string()
                    },
                    amount: Uint128::from(100000u128)
                }
            )
        );

        // case token registered, convert from source
        // ratio: 0.01
        let res = converter_contract
            .process_convert(
                &deps.as_ref().querier,
                &AssetInfo::NativeToken {
                    denom: "inj".to_string(),
                },
                Uint128::from(100000u128),
                ConvertType::FromSource,
            )
            .unwrap();
        assert_eq!(
            res,
            (
                Some(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "converter".to_string(),
                    msg: to_binary(&converter::ExecuteMsg::Convert {}).unwrap(),
                    funds: vec![Coin {
                        amount: Uint128::from(100000u128),
                        denom: "inj".to_string()
                    }]
                })),
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: Addr::unchecked("orai123")
                    },
                    amount: Uint128::from(1000u128)
                }
            )
        );

        // case token registered, convert to source
        // ratio: 0.01
        let res = converter_contract
            .process_convert(
                &deps.as_ref().querier,
                &AssetInfo::NativeToken {
                    denom: "inj".to_string(),
                },
                Uint128::from(100000u128),
                ConvertType::ToSource,
            )
            .unwrap();
        assert_eq!(
            res,
            (
                Some(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "orai123".to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: "converter".to_string(),
                        amount: Uint128::from(100000u128),
                        msg: to_binary(&converter::Cw20HookMsg::ConvertReverse {
                            from: AssetInfo::NativeToken {
                                denom: "inj".to_string(),
                            },
                        })
                        .unwrap(),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "inj".to_string()
                    },
                    amount: Uint128::from(10000000u128)
                }
            )
        )
    }
}
