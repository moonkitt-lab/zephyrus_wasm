use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{Decimal, OwnedDeps};
use cw2::set_contract_version;
use cw_storage_plus::Item;
use neutron_sdk::bindings::query::NeutronQuery;
use zephyrus_core::msgs::MigrateMsg;
use zephyrus_core::state::Constants;

use crate::migration::migrate::migrate;
use crate::migration::v0_2_0::{ConstantsV0_2_0, HydroConfigV0_2_0};
use crate::state::{CONSTANTS, CONTRACT_NAME};

#[test]
fn migrate_constants_test() {
    let mut deps: OwnedDeps<MockStorage, MockApi, MockQuerier<NeutronQuery>> = OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default().with_prefix("neutron"),
        querier: MockQuerier::<NeutronQuery>::new(&[]),
        custom_query_type: std::marker::PhantomData,
    };
    let env = mock_env();

    const OLD_CONSTANTS: Item<ConstantsV0_2_0> = Item::new("constants");

    let old_constants = ConstantsV0_2_0 {
        default_hydromancer_id: 1,
        paused_contract: false,
        hydro_config: HydroConfigV0_2_0 {
            hydro_contract_address: deps.api.addr_make("hydro_contract"),
            hydro_tribute_contract_address: deps.api.addr_make("hydro_tribute_contract"),
        },
        commission_rate: Decimal::percent(5),
        commission_recipient: deps.api.addr_make("commission_recipient"),
        min_tokens_per_vessel: 1000,
    };

    OLD_CONSTANTS
        .save(deps.as_mut().storage, &old_constants)
        .unwrap();

    // Set initial contract version to 0.2.0 to be able to migrate to the latest version
    set_contract_version(deps.as_mut().storage, CONTRACT_NAME, "0.2.0").unwrap();

    migrate(deps.as_mut(), env, MigrateMsg {}).expect("migration failed");

    let new_constants: Constants = CONSTANTS
        .load(deps.as_ref().storage)
        .expect("migrated constants missing");

    // Verify all old fields are preserved
    assert_eq!(
        new_constants.default_hydromancer_id,
        old_constants.default_hydromancer_id
    );
    assert_eq!(new_constants.paused_contract, old_constants.paused_contract);
    assert_eq!(
        new_constants.hydro_config.hydro_contract_address,
        old_constants.hydro_config.hydro_contract_address
    );
    assert_eq!(
        new_constants.hydro_config.hydro_tribute_contract_address,
        old_constants.hydro_config.hydro_tribute_contract_address
    );
    assert_eq!(new_constants.commission_rate, old_constants.commission_rate);
    assert_eq!(
        new_constants.commission_recipient,
        old_constants.commission_recipient
    );
    assert_eq!(
        new_constants.min_tokens_per_vessel,
        old_constants.min_tokens_per_vessel
    );

    // Verify new field was set to the DaoDao hydro governance address
    assert_eq!(
        new_constants
            .hydro_config
            .hydro_governance_proposal_address
            .to_string(),
        "neutron1ruwj6v94rasjkrv4h3xzrx9xnhq20md5azr537v38wms6mtj34rq23c0hq"
    );
}
