use cosmwasm_schema::cw_serde;
use cosmwasm_std::{IbcEndpoint, Uint128};
use oraiswap::asset::{Asset, AssetInfo};

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
pub struct TokenFee {
    pub token_denom: String,
    pub ratio: Ratio,
}

#[cw_serde]
pub struct RelayerFee {
    pub prefix: String,
    pub fee: Uint128,
}

#[cw_serde]
pub struct Ratio {
    pub nominator: u64,
    pub denominator: u64,
}

#[cw_serde]
pub struct MappingMetadata {
    /// asset info on local chain. Can be either cw20 or native
    pub asset_info: AssetInfo,
    pub remote_decimals: u8,
    pub asset_info_decimals: u8,
    #[serde(default)]
    pub is_mint_burn: bool,
}

#[cw_serde]
pub struct ReplyArgs {
    pub channel: String,
    pub local_receiver: String,
    pub denom: String,
    pub amount: Uint128,
}

#[cw_serde]
pub struct ConvertReplyArgs {
    pub local_receiver: String,
    pub asset: Asset,
}
