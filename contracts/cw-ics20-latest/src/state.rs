use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, IbcEndpoint, StdResult, Storage, Uint128};
use cw_controllers::Admin;
use cw_storage_plus::{Item, Map};

use crate::ContractError;

pub const ADMIN: Admin = Admin::new("admin");

pub const CONFIG: Item<Config> = Item::new("ics20_config");

// Used to pass info from the ibc_packet_receive to the reply handler
pub const REPLY_ARGS: Item<ReplyArgs> = Item::new("reply_args");

/// static info on one channel that doesn't change
pub const CHANNEL_INFO: Map<&str, ChannelInfo> = Map::new("channel_info");

/// indexed by (channel_id, denom) maintaining the balance of the channel in that currency
pub const CHANNEL_STATE: Map<(&str, &str), ChannelState> = Map::new("channel_state");

/// Every cw20 contract we allow to be sent is stored here, possibly with a gas_limit
pub const ALLOW_LIST: Map<&Addr, AllowInfo> = Map::new("allow_list");

/// custom contract that handles the native token sent from remote chain.
pub const NATIVE_ALLOW_CONTRACT: Item<Addr> = Item::new("allow_native_custom_contract");

// used when chain A (no cosmwasm) sends native token to chain B (has cosmwasm). key - original denom of chain A, in form of ibc no hash for destination port & channel - transfer/channel-0/uatom for example; value - mapping data including cw20 denom of chain B, in form: cw20:mars18vd8fpwxzck93qlwghaj6arh4p7c5n89plpqv0 for example
pub const CW20_ISC20_DENOM: Map<&str, Cw20MappingMetadata> = Map::new("cw20_ics20_mapping");

// this reverse map is used when the CW20_ISC20_DENOM is updated for simplicity (usually we can use indexing, but this is just for simplicity). TODO: change to indexing instead
pub const CW20_ISC20_DENOM_REVERSE: Map<&str, String> = Map::new("cw20_ics20_mapping_reverse");

#[cw_serde]
#[derive(Default)]
pub struct ChannelState {
    pub outstanding: Uint128,
    pub total_sent: Uint128,
}

#[cw_serde]
pub struct Config {
    pub default_timeout: u64,
    pub default_gas_limit: Option<u64>,
}

#[cw_serde]
pub struct ChannelInfo {
    /// id of this channel
    pub id: String,
    /// the remote channel/port we connect to
    pub counterparty_endpoint: IbcEndpoint,
    /// the connection this exists on (you can use to query client/consensus info)
    pub connection_id: String,
}

#[cw_serde]
pub struct AllowInfo {
    pub gas_limit: Option<u64>,
}
#[cw_serde]
pub struct Cw20MappingMetadata {
    /// denom should be in form: cw20:...
    pub cw20_denom: String,
    pub remote_decimals: u8,
}

#[cw_serde]
pub struct ReplyArgs {
    pub channel: String,
    pub denom: String,
    pub amount: Uint128,
}

pub fn increase_channel_balance(
    storage: &mut dyn Storage,
    channel: &str,
    denom: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    CHANNEL_STATE.update(storage, (channel, denom), |orig| -> StdResult<_> {
        let mut state = orig.unwrap_or_default();
        state.outstanding += amount;
        state.total_sent += amount;
        Ok(state)
    })?;
    Ok(())
}

pub fn reduce_channel_balance(
    storage: &mut dyn Storage,
    channel: &str,
    denom: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    println!("channel: {:?}", channel);
    println!("denom: {:?}", denom);
    CHANNEL_STATE.update(
        storage,
        (channel, denom),
        |orig| -> Result<_, ContractError> {
            // this will return error if we don't have the funds there to cover the request (or no denom registered)
            let mut cur = orig.ok_or(ContractError::InsufficientFunds {})?;
            cur.outstanding = cur
                .outstanding
                .checked_sub(amount)
                .or(Err(ContractError::InsufficientFunds {}))?;
            Ok(cur)
        },
    )?;
    Ok(())
}

// this is like increase, but it only "un-subtracts" (= adds) outstanding, not total_sent
// calling `reduce_channel_balance` and then `undo_reduce_channel_balance` should leave state unchanged.
pub fn undo_reduce_channel_balance(
    storage: &mut dyn Storage,
    channel: &str,
    denom: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    CHANNEL_STATE.update(storage, (channel, denom), |orig| -> StdResult<_> {
        let mut state = orig.unwrap_or_default();
        state.outstanding += amount;
        Ok(state)
    })?;
    Ok(())
}

// this is like decrease, but it only "un-add" (= adds) outstanding, not total_sent
// calling `increase_channel_balance` and then `undo_increase_channel_balance` should leave state unchanged.
pub fn undo_increase_channel_balance(
    storage: &mut dyn Storage,
    channel: &str,
    denom: &str,
    amount: Uint128,
) -> Result<(), ContractError> {
    CHANNEL_STATE.update(storage, (channel, denom), |orig| -> StdResult<_> {
        let mut state = orig.unwrap_or_default();
        state.outstanding -= amount;
        Ok(state)
    })?;
    Ok(())
}

pub fn get_key_ics20_ibc_denom(port_id: &str, channel_id: &str, denom: &str) -> String {
    format!("{}/{}/{}", port_id, channel_id, denom)
}
