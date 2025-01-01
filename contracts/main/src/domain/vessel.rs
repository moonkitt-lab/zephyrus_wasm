use cosmwasm_std::{Addr, DepsMut};
use zephyrus_core::msgs::{HydroLockId, HydromancerId, Vessel};

use crate::{
    errors::ContractError,
    state::{self, get_hydromancer},
};

type VesselClass = u64;

pub fn create_new_vessel(
    deps: DepsMut,
    vessel_id: HydroLockId,
    auto_maintenance: bool,
    vessel_class: VesselClass,
    hydromancer_id: HydromancerId,
    owner: &Addr,
) -> Result<Vessel, ContractError> {
    get_hydromancer(deps.storage, hydromancer_id)?;
    let vessel = Vessel {
        hydro_lock_id: vessel_id,
        class_period: vessel_class,
        hydromancer_id,
        auto_maintenance,
    };

    state::add_vessel(deps.storage, &vessel, owner)?;

    Ok(vessel)
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::mock_dependencies, Decimal};
    use state::Hydromancer;

    use crate::errors::ContractError;

    use super::*;

    #[test]
    fn execute_create_new_vessel_fails_if_hydromancer_does_not_exist() {
        let mut deps = mock_dependencies();
        let owner = Addr::unchecked("owner");
        let hydromancer_id = 1;
        let vessel_id = 1;
        let vessel_class = 1;
        let result = create_new_vessel(
            deps.as_mut(),
            vessel_id,
            true,
            vessel_class,
            hydromancer_id,
            &owner,
        );
        let error = result.unwrap_err();
        assert_eq!(error, ContractError::HydromancerNotFound { hydromancer_id });
    }

    #[test]
    fn execute_create_new_vessel_succeeds() {
        let mut deps = mock_dependencies();
        let hydromancer = Hydromancer {
            hydromancer_id: 1,
            address: Addr::unchecked("hydromancer"),
            name: "Hydromancer".to_string(),
            commission_rate: Decimal::from_ratio(1u128, 100u128),
        };
        state::add_hydromancer(deps.as_mut().storage, &hydromancer)
            .expect("Hydromancer should be saved");
        let owner = Addr::unchecked("owner");
        let hydromancer_id = 1;
        let vessel_id = 1;
        let vessel_class = 1;
        let vessel = create_new_vessel(
            deps.as_mut(),
            vessel_id,
            true,
            vessel_class,
            hydromancer_id,
            &owner,
        )
        .unwrap();
        let stored_vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel, stored_vessel);
    }
}
