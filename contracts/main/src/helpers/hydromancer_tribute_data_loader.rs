use std::collections::HashMap;

use cosmwasm_std::{StdResult, Storage};
use zephyrus_core::{
    msgs::{HydromancerId, RoundId, TributeId},
    state::HydromancerTribute,
};

use crate::state;

// Loader pour le contexte Query
pub trait DataLoader {
    fn load_hydromancer_tribute(
        &self,
        storage: &dyn Storage,
        hydromancer_id: u64,
        round_id: u64,
        tribute_id: u64,
    ) -> StdResult<Option<HydromancerTribute>>;
}
pub struct InMemoryDataLoader {
    pub hydromancer_tributes: HashMap<(HydromancerId, RoundId, TributeId), HydromancerTribute>,
}
impl DataLoader for InMemoryDataLoader {
    fn load_hydromancer_tribute(
        &self,
        _: &dyn Storage,
        hydromancer_id: u64,
        round_id: u64,
        tribute_id: u64,
    ) -> StdResult<Option<HydromancerTribute>> {
        Ok(self
            .hydromancer_tributes
            .get(&(hydromancer_id, round_id, tribute_id))
            .cloned())
    }
}

// Loader pour le contexte Execute
pub struct StateDataLoader;

impl DataLoader for StateDataLoader {
    fn load_hydromancer_tribute(
        &self,
        storage: &dyn Storage,
        hydromancer_id: u64,
        round_id: u64,
        tribute_id: u64,
    ) -> StdResult<Option<HydromancerTribute>> {
        state::get_hydromancer_rewards_by_tribute(storage, hydromancer_id, round_id, tribute_id)
    }
}
