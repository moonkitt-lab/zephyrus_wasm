use cosmwasm_std::{entry_point, DepsMut, Env, Response as CwResponse, StdError};
use cw2::{get_contract_version, set_contract_version};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::msgs::MigrateMsg;

use crate::{
    errors::ContractError,
    migration::unreleased,
    state::{CONTRACT_NAME, CONTRACT_VERSION},
};

type Response = CwResponse<NeutronMsg>;

#[entry_point]
pub fn migrate(mut deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let old_contract_version = check_contract_version(deps.storage)?;

    if old_contract_version == "0.2.0" {
        unreleased::migrate_constants(&mut deps)?;
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("from_version", &old_contract_version)
        .add_attribute("to_version", CONTRACT_VERSION))
}

fn check_contract_version(storage: &dyn cosmwasm_std::Storage) -> Result<String, ContractError> {
    let contract_version = get_contract_version(storage)?;

    if contract_version.version == CONTRACT_VERSION {
        return Err(ContractError::Std(StdError::generic_err(
            "Contract is already migrated to the newest version.",
        )));
    }

    Ok(contract_version.version)
}
