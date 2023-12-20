use cosmwasm_std::{Order, StdResult, Storage};
use oraiswap::asset::AssetInfo;

use crate::{
    msg::PairQuery,
    state::{ics20_denoms, MappingMetadata},
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
