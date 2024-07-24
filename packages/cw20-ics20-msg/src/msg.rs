use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, IbcEndpoint, SubMsg, Uint128};
use oraiswap::asset::AssetInfo;

use crate::amount::Amount;

/// This is the message we accept via Receive
#[cw_serde]
pub struct TransferBackMsg {
    /// the local ibc endpoint you want to send tokens back on
    pub local_channel_id: String,
    pub remote_address: String,
    /// remote denom so that we know what denom to filter when we query based on the asset info. Most likely be: oraib0x... or eth0x...
    pub remote_denom: String,
    /// How long the packet lives in seconds. If not specified, use default_timeout
    pub timeout: Option<u64>,
    /// metadata of the transfer to suit the new fungible token transfer
    pub memo: Option<String>,
}

/// This is the message we accept via Receive
#[cw_serde]
pub struct TransferBackToRemoteChainMsg {
    /// The remote chain's ibc information
    pub ibc_endpoint: IbcEndpoint,
    /// The remote address to send to.
    /// Don't use HumanAddress as this will likely have a different Bech32 prefix than we use
    /// and cannot be validated locally
    pub remote_address: String,
    /// How long the packet lives in seconds. If not specified, use default_timeout
    pub timeout: Option<u64>,
    pub metadata: Binary,
}

#[cw_serde]
pub struct AllowedInfo {
    pub contract: String,
    pub gas_limit: Option<u64>,
}

#[cw_serde]
pub struct FeeData {
    pub deducted_amount: Uint128,
    pub token_fee: Amount,
    pub relayer_fee: Amount,
}

#[cw_serde]
pub struct FollowUpMsgsData {
    pub sub_msgs: Vec<SubMsg>,
    pub follow_up_msg: String,
    pub is_success: bool,
}

#[cw_serde]
pub struct UpdatePairMsg {
    pub local_channel_id: String,
    /// native denom of the remote chain. Eg: orai
    pub denom: String,
    /// asset info of the local chain.
    pub local_asset_info: AssetInfo,
    pub remote_decimals: u8,
    pub local_asset_info_decimals: u8,
    pub is_mint_burn: Option<bool>,
}

#[cw_serde]
pub struct DeletePairMsg {
    pub local_channel_id: String,
    /// native denom of the remote chain. Eg: orai
    pub denom: String,
}
