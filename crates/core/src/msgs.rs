use crate::state::{Constants, Vessel, VesselHarbor};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Coin, Decimal};

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

#[cw_serde]
pub struct VesselsToHarbor {
    pub vessel_ids: Vec<HydroLockId>,
    pub harbor_id: HydroProposalId,
}

#[cw_serde]
pub struct VesselInfo {
    pub owner: String,
    pub auto_maintenance: bool,
    pub hydromancer_id: u64,
    pub class_period: u64,
}

#[cw_serde]
pub struct Cw721ReceiveMsg {
    pub sender: String,
    pub token_id: String,
    pub msg: Binary,
}

#[cw_serde]
pub enum ExecuteMsg {
    TakeControl {
        vessel_ids: Vec<u64>,
    },
    Unvote {
        tranche_id: TrancheId,
        vessel_ids: Vec<u64>,
    },
    UpdateVesselsClass {
        hydro_lock_ids: Vec<u64>,
        hydro_lock_duration: u64,
    },
    AutoMaintain {
        start_from_vessel_id: Option<u64>,
        limit: Option<usize>,
    },
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
    ReceiveNft(Cw721ReceiveMsg),
    ChangeHydromancer {
        tranche_id: TrancheId,
        hydromancer_id: HydromancerId,
        hydro_lock_ids: Vec<u64>,
    },
    Claim {
        round_id: u64,
        tranche_id: u64,
        vessel_ids: Vec<u64>,
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

pub const DECOMMISSION_REPLY_ID: u64 = 1;
pub const VOTE_REPLY_ID: u64 = 2;
pub const REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID: u64 = 3;

#[cw_serde]
pub struct VoteReplyPayload {
    pub tranche_id: u64,
    pub vessels_harbors: Vec<VesselsToHarbor>,
    pub steerer_id: u64,
    pub round_id: u64,
    pub user_vote: bool,
}

#[cw_serde]
pub struct RefreshTimeWeightedSharesReplyPayload {
    pub vessel_ids: Vec<HydroLockId>,
    pub target_class_period: u64,
    pub current_round_id: RoundId,
}

#[cw_serde]
pub struct DecommissionVesselsReplyPayload {
    pub previous_balances: Vec<Coin>,
    pub expected_unlocked_ids: Vec<u64>,
    pub vessel_owner: Addr,
}
