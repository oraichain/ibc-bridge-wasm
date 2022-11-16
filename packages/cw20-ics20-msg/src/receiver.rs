use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Binary, CosmosMsg, StdResult, WasmMsg};

use crate::amount::Amount;

/// Cw20ReceiveMsg should be de/serialized under `IbcWasmReceive()` variant in a ExecuteMsg
#[cw_serde]

pub struct Cw20Ics20ReceiveMsg {
    /// receiver of the token
    pub receiver: String,
    /// token from the remote chain
    pub token: Amount,
    /// the decimals of the native token, popular is 18 or 6
    pub from_decimals: u8,
    /// additional data from the memo of the IBC transfer packet
    pub data: Binary,
}

impl Cw20Ics20ReceiveMsg {
    /// serializes the message
    pub fn into_binary(self) -> StdResult<Binary> {
        let msg = ReceiverExecuteMsg::IbcWasmReceive(self);
        to_binary(&msg)
    }

    /// creates a cosmos_msg sending this struct to the named contract
    pub fn into_cosmos_msg<T: Into<String>>(self, contract_addr: T) -> StdResult<CosmosMsg> {
        let msg = self.into_binary()?;
        let execute = WasmMsg::Execute {
            contract_addr: contract_addr.into(),
            msg,
            funds: vec![],
        };
        Ok(execute.into())
    }
}

// This is just a helper to properly serialize the above message
#[cw_serde]

enum ReceiverExecuteMsg {
    IbcWasmReceive(Cw20Ics20ReceiveMsg),
}
