use cosmwasm_schema::write_api;

use receiver_contract::contract::{ExecuteMsg, InitMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InitMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
