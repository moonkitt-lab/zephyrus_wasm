use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;

use crate::msgs::UserControl;

pub type UserId = u64;
pub type HydromancerId = u64;
pub type HydroLockId = u64; // This doesn't use a sequence, as we use lock_id returned by Hydro

#[derive(Copy)]
#[cw_serde]
pub struct Vessel {
    pub hydro_lock_id: HydroLockId,
    pub tokenized_share_record_id: Option<u64>,
    pub class_period: u64,
    pub auto_maintenance: bool,
    pub hydromancer_id: u64,
    pub owner_id: UserId,
}

#[cw_serde]
pub struct Constants {
    pub default_hydromancer_id: HydromancerId,
    pub paused_contract: bool,
    pub hydro_config: HydroConfig,
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
