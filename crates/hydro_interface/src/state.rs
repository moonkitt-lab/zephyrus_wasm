use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, QuerierWrapper, StdResult, Timestamp};
use cw_storage_plus::Map;

// First, define the same types as in the Hydro contract
#[cw_serde]
pub struct LockEntry {
    pub lock_id: u64,
    pub funds: Coin,
    pub lock_start: Timestamp,
    pub lock_end: Timestamp,
}

// Create the same Map definition
pub const LOCKS_MAP: Map<(Addr, u64), LockEntry> = Map::new("locks_map");

// Function to query multiple lock entries
pub fn query_lock_entries(
    querier: &QuerierWrapper,
    hydro_contract: Addr,
    owner: Addr,
    lock_ids: &[u64],
) -> StdResult<Vec<(u64, LockEntry)>> {
    let mut entries = vec![];

    for lock_id in lock_ids {
        if let Some(entry) =
            LOCKS_MAP.query(querier, hydro_contract.clone(), (owner.clone(), *lock_id))?
        {
            entries.push((*lock_id, entry));
        }
    }

    Ok(entries)
}
