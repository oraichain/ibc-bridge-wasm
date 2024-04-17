use cosmwasm_std::{Api, Env, Order, StdResult, Storage};
use cw20_ics20_msg::helper::{denom_to_asset_info, parse_ibc_wasm_port_id};
use oraiswap::asset::AssetInfo;
use sha256::digest;

use crate::{
    msg::PairQuery,
    state::{get_key_ics20_ibc_denom, ics20_denoms, MappingMetadata},
};

pub fn get_mappings_from_asset_info(
    storage: &dyn Storage,
    asset_info: AssetInfo,
) -> StdResult<Vec<PairQuery>> {
    let pair_mappings = ics20_denoms()
        .idx
        .asset_info
        .prefix(asset_info.to_string())
        .range(storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<(String, MappingMetadata)>>>()?;

    let pair_queries = pair_mappings
        .into_iter()
        .map(|pair| PairQuery {
            key: pair.0,
            pair_mapping: pair.1,
        })
        .collect();
    Ok(pair_queries)
}

pub fn get_destination_info_on_orai(
    storage: &dyn Storage,
    api: &dyn Api,
    env: &Env,
    destination_channel: &str,
    destination_denom: &str,
) -> (AssetInfo, Option<PairQuery>) {
    // destination is Oraichain
    if destination_channel.is_empty() {
        return (denom_to_asset_info(api, destination_denom), None);
    }

    // case 1: port is ibc wasm, must be registered in mapping
    let ibc_denom = get_key_ics20_ibc_denom(
        &parse_ibc_wasm_port_id(env.contract.address.as_str()),
        destination_channel,
        destination_denom,
    );
    if let Ok(pair_mapping) = ics20_denoms().load(storage, &ibc_denom) {
        return (
            pair_mapping.asset_info.clone(),
            Some(PairQuery {
                key: ibc_denom,
                pair_mapping,
            }),
        );
    }

    // case 2: port is transfer
    let ibc_denom = format!(
        "ibc/{}",
        digest(get_key_ics20_ibc_denom(
            "transfer",
            destination_channel,
            destination_denom
        ))
        .to_uppercase()
    );

    (AssetInfo::NativeToken { denom: ibc_denom }, None)
}
