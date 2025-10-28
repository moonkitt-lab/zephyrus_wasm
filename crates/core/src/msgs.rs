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
pub type TributeId = u64;

#[cw_serde]
pub struct InstantiateMsg {
    pub hydro_contract_address: String,
    pub tribute_contract_address: String,
    pub whitelist_admins: Vec<String>,
    pub commission_rate: Decimal,
    pub default_hydromancer_name: String,
    pub default_hydromancer_commission_rate: Decimal,
    pub default_hydromancer_address: String,
    pub commission_recipient: String,
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
/// Contract execution messages.
///
/// Each variant describes a possible external action.
#[cw_serde]
pub enum ExecuteMsg {
    /// Executable message for Zephyrus users that allows the caller
    /// to reclaim control of the specified vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to reclaim control of.
    TakeControl { vessel_ids: Vec<u64> },
    /// Executable message for Zephyrus for users or hydromancers
    /// to unvote from the specified tranche and vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the hydromancer or the user controlling every vessel he wishes to unvote.
    Unvote {
        tranche_id: TrancheId,
        vessel_ids: Vec<u64>,
    },
    /// Executable message for Zephyrus users that allows the caller
    /// to update the class period of the specified vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to update the class period of.
    /// - The new end of lock duration must be in the valid durations.
    /// - The new end of lock duration must be greater than the current end of lock duration of every vessel.
    UpdateVesselsClass {
        hydro_lock_ids: Vec<u64>,
        hydro_lock_duration: u64,
    },
    /// Executable message for anybody
    /// to auto-maintain the limit number of vessels with auto_maintenance true
    /// Anybody can call this function.
    /// Preconditions:
    /// - The contract must not be paused.
    AutoMaintain {
        start_from_vessel_id: Option<u64>,
        limit: Option<usize>,
    },
    /// Executable message for Zephyrus users that allows the caller
    /// to modify the auto_maintenance of the specified vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to modify the auto_maintenance of.
    ModifyAutoMaintenance {
        hydro_lock_ids: Vec<u64>,
        auto_maintenance: bool,
    },
    /// Executable message for admins
    /// to pause the contract
    /// Preconditions:
    /// - The caller must be an admin.
    /// - The contract must not be paused.
    PauseContract {},
    /// Executable message for admins
    /// to unpause the contract
    /// Preconditions:
    /// - The caller must be an admin.
    /// - The contract must be paused.
    UnpauseContract {},
    /// Executable message for users
    /// to decommission the specified vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to decommission.
    /// - Every vessel should have a lock end < now (block time)
    DecommissionVessels { hydro_lock_ids: Vec<u64> },
    /// Executable message for Zephyrus for hydromancers
    /// to vote from the specified tranche and vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the hydromancer and all the vessels should be controlled by the hydromancer.
    /// - No vessels duplicates in the harbors.
    /// - No harbors duplicates.
    HydromancerVote {
        tranche_id: TrancheId,
        vessels_harbors: Vec<VesselsToHarbor>,
    },
    /// Executable message for Zephyrus for users
    /// to vote from the specified tranche and vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the user and all the vessels should be controlled by the user.
    /// - No vessels duplicates in the harbors.
    /// - No harbors duplicates.
    UserVote {
        tranche_id: TrancheId,
        vessels_harbors: Vec<VesselsToHarbor>,
    },
    /// Executable message by hydro contract
    /// to create a vessel when a NFT is received from hydro contract
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the hydro contract.
    ReceiveNft(Cw721ReceiveMsg),
    /// Executable message for Zephyrus users
    /// to change the hydromancer of the specified vessels (provided as parameters).
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to change the hydromancer of.
    /// - The new hydromancer should exist.
    ChangeHydromancer {
        tranche_id: TrancheId,
        hydromancer_id: HydromancerId,
        hydro_lock_ids: Vec<u64>,
    },
    /// Executable message for Zephyrus users and hydromancers
    /// to claim the specified vessels rewards (provided as parameters) and commissions if caller is a hydromancer
    /// Preconditions:
    /// - The contract must not be paused.
    /// - The caller must be the owner of every vessel they wish to claim rewards for, hydromancer can claim commissions with empty vessel_ids.
    /// - The round should be completed.
    Claim {
        round_id: u64,
        tranche_id: u64,
        vessel_ids: Vec<u64>,
        tribute_ids: Vec<u64>,
    },
    /// Executable message for admins
    /// to update the commission rate
    /// Preconditions:
    /// - The caller must be an admin.
    /// - The new commission rate must be less than 1 (100%).
    UpdateCommissionRate { new_commission_rate: Decimal },
    /// Executable message for admins
    /// to update the commission recipient address
    /// Preconditions:
    /// - The caller must be an admin.
    /// - The new commission recipient must be a valid address.
    UpdateCommissionRecipient { new_commission_recipient: String },

    /// Executable message for admins
    /// to set the admin addresses
    /// Preconditions:
    /// - The caller must be an admin.
    /// - The admin addresses must be valid addresses.
    /// - intersection of new admin addresses and existing admin addresses must not be empty.
    SetAdminAddresses { admins: Vec<String> },
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
    #[returns(VesselsRewardsResponse)]
    VesselsRewards {
        user_address: String,
        round_id: u64,
        tranche_id: u64,
        vessel_ids: Vec<u64>,
    },
    #[returns(VotedProposalsResponse)]
    VotedProposals { round_id: u64 },
}

#[cw_serde]
pub struct MigrateMsg {}

pub const DECOMMISSION_REPLY_ID: u64 = 1;
pub const VOTE_REPLY_ID: u64 = 2;
pub const REFRESH_TIME_WEIGHTED_SHARES_REPLY_ID: u64 = 3;
pub const CLAIM_TRIBUTE_REPLY_ID: u64 = 4;

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

#[cw_serde]
pub struct ClaimTributeReplyPayload {
    pub proposal_id: u64,
    pub tribute_id: u64,
    pub round_id: u64,
    pub tranche_id: u64,
    pub amount: Coin,
    pub balance_before_claim: Coin,
    pub vessels_owner: Addr,
    pub vessel_ids: Vec<u64>,
}

#[cw_serde]
pub struct RewardInfo {
    pub coin: Coin,
    pub proposal_id: u64,
    pub tribute_id: u64,
}

#[cw_serde]
pub struct VesselsRewardsResponse {
    pub round_id: u64,
    pub tranche_id: u64,
    pub rewards: Vec<RewardInfo>,
}

#[cw_serde]
pub struct VotedProposalsResponse {
    pub voted_proposals: Vec<u64>,
}
