use crate::state::{Constants, Vessel, VesselHarbor};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Decimal;

pub type UserId = u64;
pub type HydromancerId = u64;
pub type HydroLockId = u64; // This doesn't use a sequence, as we use lock_id returned by Hydro
pub type HydroProposalId = u64;
pub type TrancheId = u64;
pub type TributeId = u64;
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
    UserVote {
        tranche_id: TrancheId,
        vessels_harbors: Vec<VesselsToHarbor>,
    },
    ChangeHydromancer {
        tranche_id: TrancheId,
        hydromancer_id: HydromancerId,
        hydro_lock_ids: Vec<u64>,
    },
    Claim {
        tranche_id: TrancheId,
        round_ids: Vec<RoundId>,
    },
}

#[cw_serde]
pub struct VotingPowerResponse {
    pub voting_power: u64,
}

#[cw_serde]
pub struct VesselHarborInfo {
    pub vessel_to_harbor: Option<VesselHarbor>,
    pub vessel_id: u64,
    pub harbor_id: Option<u64>,
}

#[cw_serde]
pub struct VesselHarborResponse {
    pub vessels_harbor_info: Vec<VesselHarborInfo>,
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
    #[returns(VesselHarborResponse)]
    VesselsHarbor {
        tranche_id: u64,
        round_id: u64,
        lock_ids: Vec<u64>,
    },
}

#[cw_serde]
pub struct MigrateMsg {}
