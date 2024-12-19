use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Decimal;

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
pub enum ExecuteMsg {
    // TODO: Determine message variants
    BuildVessel {
        lock_duration: u64,
        auto_maintenance: bool,
        hydromancer_id: u64,
    },
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
