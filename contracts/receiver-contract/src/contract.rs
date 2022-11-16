use cosmwasm_schema::{cw_serde, QueryResponses};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, IbcMsg, IbcQuery, MessageInfo, Order,
    PortIdResponse, Response, StdError, StdResult,
};
use cw20_ics20_msg::receiver::Cw20Ics20ReceiveMsg;
use cw_storage_plus::Item;

use crate::error::ContractError;

#[cw_serde]
pub struct InitMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    IbcWasmReceive(Cw20Ics20ReceiveMsg),
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {}

pub const COUNT: Item<u128> = Item::new("count");

// version info for migration info

#[entry_point]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InitMsg,
) -> Result<Response, ContractError> {
    COUNT.save(deps.storage, &1u128)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::IbcWasmReceive(msg) => {
            let count = COUNT.load(deps.storage)?;
            COUNT.save(deps.storage, &(count + 1))?;

            let mut res: Response = Response::default()
                .add_attribute("receive_msg_decimals", msg.from_decimals.to_string());
            if let Some(memo) = msg.memo {
                res = res.add_attribute("memo", memo);
            }
            Ok(res)
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    Ok(Binary::default())
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::new())
}
