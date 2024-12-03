use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    // TODO: Determine message variants
    BuildVessel {},
}

#[cw_serde]
pub struct VotingPowerResponse {
    pub voting_power: u64,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // TODO: Determine message variants and response types
    #[returns(VotingPowerResponse)]
    VotingPower {},
}

#[cw_serde]
pub struct MigrateMsg {}
