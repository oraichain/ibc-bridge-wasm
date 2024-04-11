use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, Binary, Coin, CosmosMsg, Decimal, StdError, StdResult, Uint128,
    WasmMsg,
};
use cw20::{Cw20CoinVerified, Cw20ExecuteMsg};
use oraiswap::asset::AssetInfo;
use std::convert::TryInto;

#[cw_serde]
pub enum Amount {
    Native(Coin),
    // FIXME? USe Cw20CoinVerified, and validate cw20 addresses
    Cw20(Cw20CoinVerified),
}

impl Amount {
    pub fn from_parts(denom: String, amount: Uint128) -> Self {
        if denom.starts_with("cw20:") {
            let address = Addr::unchecked(&denom[5..]);
            Amount::Cw20(Cw20CoinVerified { address, amount })
        } else {
            Amount::Native(Coin { denom, amount })
        }
    }

    // return struct should copy instead of ref
    pub fn cw20(amount: Uint128, address: Addr) -> Self {
        Amount::Cw20(Cw20CoinVerified { address, amount })
    }

    pub fn native(amount: Uint128, denom: String) -> Self {
        Amount::Native(Coin { denom, amount })
    }

    pub fn into_asset_info(&self, api: &dyn Api) -> StdResult<AssetInfo> {
        match self {
            Amount::Native(coin) => Ok(AssetInfo::NativeToken {
                denom: coin.denom.clone(),
            }),
            Amount::Cw20(cw20_coin) => Ok(AssetInfo::Token {
                contract_addr: api.addr_validate(cw20_coin.address.as_str())?,
            }),
        }
    }
}

impl Amount {
    pub fn denom(&self) -> String {
        match self {
            Amount::Native(c) => c.denom.clone(),
            Amount::Cw20(c) => format!("cw20:{}", c.address.as_str()),
        }
    }

    // this returns original cw20 address if it's cw20
    pub fn raw_denom(&self) -> String {
        match self {
            Amount::Native(c) => c.denom.clone(),
            Amount::Cw20(c) => c.address.to_string(),
        }
    }

    pub fn amount(&self) -> Uint128 {
        match self {
            Amount::Native(c) => c.amount,
            Amount::Cw20(c) => c.amount,
        }
    }

    /// convert the amount into u64
    pub fn u64_amount(&self) -> Result<u64, StdError> {
        Ok(self
            .amount()
            .u128()
            .try_into()
            .map_err(|_| StdError::generic_err("error casting to u64 from u128".to_string()))?)
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Amount::Native(c) => c.amount.is_zero(),
            Amount::Cw20(c) => c.amount.is_zero(),
        }
    }

    pub fn send_amount(&self, recipient: String, msg: Option<Binary>) -> CosmosMsg {
        match self.to_owned() {
            Amount::Native(coin) => BankMsg::Send {
                to_address: recipient,
                amount: vec![coin],
            }
            .into(),
            Amount::Cw20(coin) => {
                let msg_cw20 = if let Some(msg) = msg {
                    Cw20ExecuteMsg::Send {
                        contract: recipient,
                        amount: coin.amount,
                        msg,
                    }
                } else {
                    Cw20ExecuteMsg::Transfer {
                        recipient: recipient.clone(),
                        amount: coin.amount,
                    }
                };

                WasmMsg::Execute {
                    contract_addr: coin.address.to_string(),
                    msg: to_binary(&msg_cw20).unwrap(),
                    funds: vec![],
                }
                .into()
            }
        }
    }
}

impl Amount {
    pub fn checked_add(&self, add_amount: Uint128) -> Self {
        let amount = self.amount();
        Amount::from_parts(
            self.denom(),
            amount.checked_add(add_amount).unwrap_or(amount),
        )
    }
}

fn mul_ratio_decimal(amount: Uint128, ratio: Decimal) -> StdResult<Uint128> {
    let result = Decimal::one()
        .checked_mul(ratio)
        .map_err(|err| StdError::generic_err(err.to_string()))
        .map(|coeff| amount * coeff)?;
    if result.is_zero() {
        return Err(StdError::generic_err(
            "Converting decimals results in a zero amount. Revert this transaction!",
        ));
    }
    Ok(result)
}

pub fn convert_remote_to_local(
    amount: Uint128,
    remote_decimals: u8,
    local_decimals: u8,
) -> StdResult<Uint128> {
    mul_ratio_decimal(
        amount,
        Decimal::from_ratio(
            10u128.pow(local_decimals as u32),
            10u128.pow(remote_decimals as u32),
        ),
    )
}

pub fn convert_local_to_remote(
    amount: Uint128,
    remote_decimals: u8,
    local_decimals: u8,
) -> StdResult<Uint128> {
    mul_ratio_decimal(
        amount,
        Decimal::from_ratio(
            10u128.pow(remote_decimals as u32),
            10u128.pow(local_decimals as u32),
        ),
    )
}

#[cfg(test)]
mod tests {

    use cosmwasm_std::{testing::mock_dependencies, Addr};

    use super::*;

    #[test]
    pub fn test_div_ratio_decimal() {
        let new_amount = convert_local_to_remote(Uint128::from(10u128), 18, 6).unwrap();
        assert_eq!(new_amount, Uint128::from(10000000000000u128));
        let new_amount = convert_remote_to_local(Uint128::from(1000000000000u128), 18, 6).unwrap();
        assert_eq!(new_amount, Uint128::from(1u128))
    }

    #[test]
    pub fn test_into_asset_info() {
        let deps = mock_dependencies();
        let amount = Amount::cw20(1u128.into(), Addr::unchecked("addr"));
        assert_eq!(
            amount.into_asset_info(deps.as_ref().api).unwrap(),
            AssetInfo::Token {
                contract_addr: Addr::unchecked("addr")
            }
        );
        let amount = Amount::native(1u128.into(), "native".to_string());
        assert_eq!(
            amount.into_asset_info(deps.as_ref().api).unwrap(),
            AssetInfo::NativeToken {
                denom: "native".to_string()
            }
        )
    }

    #[test]
    pub fn test_checked_add() {
        assert_eq!(
            Amount::cw20(1u128.into(), Addr::unchecked("addr")).checked_add(Uint128::one()),
            Amount::cw20(2u128.into(), Addr::unchecked("addr"))
        );

        assert_eq!(
            Amount::cw20(1u128.into(), Addr::unchecked("addr")).checked_add(Uint128::MAX),
            Amount::cw20(1u128.into(), Addr::unchecked("addr"))
        )
    }
}
