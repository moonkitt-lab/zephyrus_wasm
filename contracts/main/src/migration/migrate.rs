use cosmwasm_std::{entry_point, DepsMut, Env, Response as CwResponse, StdError};
use cw2::{get_contract_version, set_contract_version};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::msgs::MigrateMsg;

use crate::{
    errors::ContractError,
    state::{self, CONTRACT_NAME, CONTRACT_VERSION},
};

type Response = CwResponse<NeutronMsg>;

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    check_contract_version(deps.storage)?;

    // Initialize the new hydro governance proposal address
    let mut constants = state::get_constants(deps.storage)?;
    constants.hydro_config.hydro_governance_proposal_address = deps
        .api
        .addr_validate("neutron1lefyfl55ntp7j58k8wy7x3yq9dngsj73s5syrreq55hu4xst660s5p2jtj")?;
    state::update_constants(deps.storage, constants)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("contract_version", CONTRACT_VERSION))
}

fn check_contract_version(storage: &dyn cosmwasm_std::Storage) -> Result<(), ContractError> {
    let contract_version = get_contract_version(storage)?;

    if contract_version.version == CONTRACT_VERSION {
        return Err(ContractError::Std(StdError::generic_err(
            "Contract is already migrated to the newest version.",
        )));
    }

    Ok(())
}
