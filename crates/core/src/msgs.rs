use crate::state::{Constants, Vessel};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};

pub type UserId = u64;
pub type HydromancerId = u64;
pub type HydroLockId = u64; // This doesn't use a sequence, as we use lock_id returned by Hydro
pub type HydroProposalId = u64;
pub type TrancheId = u64;
pub type RoundId = u64;
pub type UserControl = bool;

#[cw_serde]
pub struct InstantiateMsg {
    pub hydro_contract_address: String,
    pub tribute_contract_address: String,
    pub whitelist_admins: Vec<String>,
    pub default_hydromancer_name: String,
    pub default_hydromancer_commission_rate: Decimal,
    pub default_hydromancer_address: String,
}

#[derive(Copy)]
#[cw_serde]
pub struct BuildVesselParams {
    pub lock_duration: u64,
    pub auto_maintenance: bool,
    pub hydromancer_id: u64,
}

#[cw_serde]
pub struct VesselsToHarbor {
    pub vessel_ids: Vec<HydroLockId>,
    pub harbor_id: HydroProposalId,
}

#[cw_serde]
pub enum ExecuteMsg {
    // TODO: Determine message variants
    BuildVessel {
        vessels: Vec<BuildVesselParams>,
        receiver: Option<String>,
    },
    UpdateVesselsClass {
        hydro_lock_ids: Vec<u64>,
        hydro_lock_duration: u64,
    },
    AutoMaintain {},
    ModifyAutoMaintenance {
        hydro_lock_ids: Vec<u64>,
        auto_maintenance: bool,
    },
    PauseContract {},
    UnpauseContract {},
    DecommissionVessels {
        hydro_lock_ids: Vec<u64>,
    },
    HydromancerVote {
        tranche_id: TrancheId,
        vessels_harbors: Vec<VesselsToHarbor>,
    },
}

#[cw_serde]
pub struct VotingPowerResponse {
    pub voting_power: u64,
}

#[cw_serde]
pub struct VesselsResponse {
    pub vessels: Vec<Vessel>,
    pub start_index: usize,
    pub limit: usize,
    pub total: usize,
}

#[cw_serde]
pub struct ConstantsResponse {
    pub constants: Constants,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // TODO: Determine message variants and response types
    #[returns(VotingPowerResponse)]
    VotingPower {},
    #[returns(VesselsResponse)]
    VesselsByOwner {
        owner: String,
        start_index: Option<usize>,
        limit: Option<usize>,
    },
    #[returns(VesselsResponse)]
    VesselsByHydromancer {
        hydromancer_addr: String,
        start_index: Option<usize>,
        limit: Option<usize>,
    },
    #[returns(ConstantsResponse)]
    Constants {},
}

#[cw_serde]
pub struct MigrateMsg {}
