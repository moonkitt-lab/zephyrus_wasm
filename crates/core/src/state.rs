use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal};

use crate::msgs::UserControl;

pub type UserId = u64;
pub type HydromancerId = u64;
pub type HydroLockId = u64; // This doesn't use a sequence, as we use lock_id returned by Hydro

#[cw_serde]
pub struct Vessel {
    pub hydro_lock_id: HydroLockId,
    pub tokenized_share_record_id: Option<u64>,
    pub class_period: u64,
    pub auto_maintenance: bool,
    pub hydromancer_id: Option<u64>,
    pub owner_id: UserId,
}

impl Vessel {
    pub fn is_under_user_control(&self) -> bool {
        self.hydromancer_id.is_none()
    }
}

#[cw_serde]
pub struct VesselSharesInfo {
    pub time_weighted_shares: u128,
    pub token_group_id: String,
    pub locked_rounds: u64,
}

#[cw_serde]
pub struct Constants {
    pub default_hydromancer_id: HydromancerId,
    pub paused_contract: bool,
    pub hydro_config: HydroConfig,
    pub commission_rate: Decimal,
    pub commission_recipient: Addr,
    pub min_tokens_per_vessel: u128,
}

#[cw_serde]
pub struct VesselHarbor {
    pub user_control: UserControl,
    pub steerer_id: u64,
    pub hydro_lock_id: HydroLockId,
}

#[cw_serde]
pub struct HydroConfig {
    pub hydro_contract_address: Addr,
    pub hydro_tribute_contract_address: Addr,
}

#[cw_serde]
pub struct HydromancerTribute {
    pub rewards_for_users: Coin,
    pub commission_for_hydromancer: Coin,
}
