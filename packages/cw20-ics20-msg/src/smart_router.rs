use cosmwasm_schema::cw_serde;
use cosmwasm_std::{QuerierWrapper, StdError, StdResult, Uint128};
use oraiswap::{
    asset::AssetInfo,
    router::{RouterController, SwapOperation},
};

const OFFER_AMOUNT_DEFAULT: u128 = 1000000;

#[cw_serde]
pub struct SmartRouterController(pub String);

#[cw_serde]
pub struct GetSmartRouteResponse {
    pub swap_ops: Vec<SwapOperation>,
    pub actual_minimum_receive: Uint128,
}

#[cw_serde]
pub enum SmartRouterQueryMsg {
    GetSmartRoute {
        input_info: AssetInfo,
        output_info: AssetInfo,
        offer_amount: Uint128,
    },
}

impl SmartRouterController {
    pub fn addr(&self) -> String {
        self.0.clone()
    }

    pub fn build_swap_operations(
        &self,
        querier: &QuerierWrapper,
        swap_router: &RouterController,
        offer_asset: AssetInfo,
        ask_asset: AssetInfo,
        amount: Option<Uint128>,
        tmp_denom: String,
    ) -> StdResult<GetSmartRouteResponse> {
        let offer_amount = amount.unwrap_or(Uint128::from(OFFER_AMOUNT_DEFAULT));

        let res = match querier.query_wasm_smart::<GetSmartRouteResponse>(
            self.addr(),
            &SmartRouterQueryMsg::GetSmartRoute {
                input_info: offer_asset.clone(),
                output_info: ask_asset.clone(),
                offer_amount: offer_amount.clone(),
            },
        ) {
            Ok(val) => val,
            Err(_) => GetSmartRouteResponse {
                swap_ops: vec![],
                actual_minimum_receive: Uint128::zero(),
            },
        };

        if res.swap_ops.len() == 0 {
            if ask_asset.eq(&offer_asset) {
                return Ok(GetSmartRouteResponse {
                    swap_ops: vec![],
                    actual_minimum_receive: amount.unwrap_or_default(),
                });
            } else {
                let tmp_asset_info = AssetInfo::NativeToken { denom: tmp_denom };
                let mut swap_operations = vec![];

                if offer_asset.ne(&tmp_asset_info) {
                    swap_operations.push(SwapOperation::OraiSwap {
                        offer_asset_info: offer_asset,
                        ask_asset_info: tmp_asset_info.clone(),
                    })
                }
                if ask_asset.ne(&tmp_asset_info) {
                    swap_operations.push(SwapOperation::OraiSwap {
                        offer_asset_info: tmp_asset_info.clone(),
                        ask_asset_info: ask_asset,
                    });
                }

                match swap_router.simulate_swap(querier, offer_amount, swap_operations.clone()) {
                    Ok(val) => {
                        return Ok(GetSmartRouteResponse {
                            swap_ops: swap_operations,
                            actual_minimum_receive: val.amount,
                        })
                    }
                    Err(err) => {
                        return Err(StdError::generic_err(format!(
                            "Cannot simulate swap with ops: {:?} with error: {:?}",
                            swap_operations,
                            err.to_string()
                        )));
                    }
                }
            }
        }

        Ok(res)
    }
}
