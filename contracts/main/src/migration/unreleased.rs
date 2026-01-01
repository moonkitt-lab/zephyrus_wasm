use cosmwasm_std::{DepsMut, Response as CwResponse};
use cw_storage_plus::Item;
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::state::{Constants, HydroConfig};

use crate::{errors::ContractError, migration::v0_2_0::ConstantsV0_2_0, state::CONSTANTS};

type Response = CwResponse<NeutronMsg>;

pub fn migrate_constants(deps: &mut DepsMut) -> Result<Response, ContractError> {
    const OLD_CONSTANTS: Item<ConstantsV0_2_0> = Item::new("constants");

    let old_constants = OLD_CONSTANTS.load(deps.storage)?;

    let new_constants = Constants {
        default_hydromancer_id: old_constants.default_hydromancer_id,
        paused_contract: old_constants.paused_contract,
        hydro_config: HydroConfig {
            hydro_contract_address: old_constants.hydro_config.hydro_contract_address,
            hydro_tribute_contract_address: old_constants
                .hydro_config
                .hydro_tribute_contract_address,
            // Hydro governance proposal address on mainnet (not available on devnet/testnet)
            hydro_governance_proposal_address: deps.api.addr_validate(
                "neutron1ruwj6v94rasjkrv4h3xzrx9xnhq20md5azr537v38wms6mtj34rq23c0hq",
            )?,
        },
        commission_rate: old_constants.commission_rate,
        commission_recipient: old_constants.commission_recipient,
        min_tokens_per_vessel: old_constants.min_tokens_per_vessel,
    };

    CONSTANTS.save(deps.storage, &new_constants)?;

    Ok(Response::new().add_attribute("action", "migrate_constants"))
}
