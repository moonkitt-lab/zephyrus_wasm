use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal};
use zephyrus_core::state::HydromancerId;

#[cw_serde]
pub struct HydroConfigV0_2_0 {
    pub hydro_contract_address: Addr,
    pub hydro_tribute_contract_address: Addr,
}

#[cw_serde]
pub struct ConstantsV0_2_0 {
    pub default_hydromancer_id: HydromancerId,
    pub paused_contract: bool,
    pub hydro_config: HydroConfigV0_2_0,
    pub commission_rate: Decimal,
    pub commission_recipient: Addr,
    pub min_tokens_per_vessel: u128,
}
