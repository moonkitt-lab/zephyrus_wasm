#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::MockApi, Addr, Decimal};
    use zephyrus_core::state::{Constants, HydroConfig, Vessel, VesselHarbor};

    use crate::{
        state::{
            add_time_weighted_shares_to_hydromancer, add_time_weighted_shares_to_proposal,
            add_time_weighted_shares_to_proposal_for_hydromancer, add_vessel, add_vessel_to_harbor,
            add_vessel_to_hydromancer, are_vessels_controlled_by_hydromancer, are_vessels_owned_by,
            change_vessel_hydromancer, extract_vessels_not_controlled_by_hydromancer,
            get_all_hydromancers, get_constants, get_harbor_of_vessel, get_hydromancer,
            get_hydromancer_id_by_address, get_hydromancer_proposal_time_weighted_shares,
            get_hydromancer_time_weighted_shares_by_round, get_proposal_time_weighted_shares,
            get_user_id, get_user_id_by_address, get_vessel, get_vessel_harbor,
            get_vessel_ids_auto_maintained_by_class, get_vessel_shares_info,
            get_vessel_to_harbor_by_harbor_id, get_vessels_by_hydromancer, get_vessels_by_ids,
            get_vessels_by_owner, has_vessel_shares_info, hydromancer_exists, initialize_sequences,
            insert_new_hydromancer, insert_new_user, is_hydromancer_tws_complete,
            is_tokenized_share_record_used, is_vessel_owned_by, is_vessel_used_under_user_control,
            is_whitelisted_admin, iterate_vessels_with_predicate, mark_hydromancer_tws_complete,
            modify_auto_maintenance, remove_vessel, remove_vessel_from_hydromancer,
            remove_vessel_harbor, save_vessel, save_vessel_info_snapshot,
            substract_time_weighted_shares_from_hydromancer,
            substract_time_weighted_shares_from_proposal,
            substract_time_weighted_shares_from_proposal_for_hydromancer, take_control_of_vessels,
            update_constants, update_whitelist_admins, vessel_exists,
        },
        testing_mocks::mock_dependencies,
    };

    fn make_valid_addr(addr: &str) -> Addr {
        MockApi::default().addr_make(addr)
    }

    fn setup_basic_state(storage: &mut dyn cosmwasm_std::Storage) {
        initialize_sequences(storage).unwrap();

        let constants = Constants {
            default_hydromancer_id: 0,
            paused_contract: false,
            hydro_config: HydroConfig {
                hydro_contract_address: make_valid_addr("hydro"),
                hydro_tribute_contract_address: make_valid_addr("tribute"),
            },
            commission_rate: "0.1".parse().unwrap(),
            commission_recipient: make_valid_addr("commission_recipient"),
        };
        update_constants(storage, constants).unwrap();

        let whitelist_admins = vec![make_valid_addr("admin1"), make_valid_addr("admin2")];
        update_whitelist_admins(storage, whitelist_admins).unwrap();
    }

    #[test]
    fn test_initialize_sequences() {
        let mut deps = mock_dependencies();
        let result = initialize_sequences(deps.as_mut().storage);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_and_get_constants() {
        let mut deps = mock_dependencies();
        let constants = Constants {
            default_hydromancer_id: 1,
            paused_contract: true,
            hydro_config: HydroConfig {
                hydro_contract_address: make_valid_addr("hydro_test"),
                hydro_tribute_contract_address: make_valid_addr("tribute_test"),
            },
            commission_rate: "0.1".parse().unwrap(),
            commission_recipient: make_valid_addr("commission_recipient"),
        };

        let result = update_constants(deps.as_mut().storage, constants.clone());
        assert!(result.is_ok());

        let retrieved_constants = get_constants(deps.as_ref().storage);
        assert!(retrieved_constants.is_ok());
        let retrieved = retrieved_constants.unwrap();
        assert_eq!(retrieved.default_hydromancer_id, 1);
        assert_eq!(retrieved.paused_contract, true);
        assert_eq!(
            retrieved.hydro_config.hydro_contract_address,
            make_valid_addr("hydro_test")
        );
    }

    #[test]
    fn test_update_and_check_whitelist_admins() {
        let mut deps = mock_dependencies();
        let admin1 = make_valid_addr("admin1");
        let admin2 = make_valid_addr("admin2");
        let admins = vec![admin1.clone(), admin2.clone()];

        let result = update_whitelist_admins(deps.as_mut().storage, admins);
        assert!(result.is_ok());

        assert!(is_whitelisted_admin(deps.as_ref().storage, &admin1).unwrap());
        assert!(is_whitelisted_admin(deps.as_ref().storage, &admin2).unwrap());

        let non_admin = make_valid_addr("user");
        assert!(!is_whitelisted_admin(deps.as_ref().storage, &non_admin).unwrap());
    }

    #[test]
    fn test_insert_new_user() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user_address = make_valid_addr("user1");
        let result = insert_new_user(deps.as_mut().storage, user_address.clone());
        assert!(result.is_ok());
        let user_id = result.unwrap();
        assert_eq!(user_id, 0);

        // Test getting user ID by address
        let retrieved_id = get_user_id_by_address(deps.as_ref().storage, user_address.clone());
        assert!(retrieved_id.is_ok());
        assert_eq!(retrieved_id.unwrap(), user_id);

        // Test duplicate user insertion
        let duplicate_result = insert_new_user(deps.as_mut().storage, user_address);
        assert!(duplicate_result.is_err());
        assert!(duplicate_result
            .unwrap_err()
            .to_string()
            .contains("already exists"));
    }

    #[test]
    fn test_insert_multiple_users() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");

        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();

        assert_eq!(user1_id, 0);
        assert_eq!(user2_id, 1);

        assert_eq!(
            get_user_id_by_address(deps.as_ref().storage, user1).unwrap(),
            0
        );
        assert_eq!(
            get_user_id_by_address(deps.as_ref().storage, user2).unwrap(),
            1
        );
    }

    #[test]
    fn test_get_user_id() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user_address = make_valid_addr("user1");
        let user_id = insert_new_user(deps.as_mut().storage, user_address.clone()).unwrap();

        let result = get_user_id(deps.as_ref().storage, &user_address);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), user_id);

        // Test non-existent user
        let non_existent = make_valid_addr("non_existent");
        let result = get_user_id(deps.as_ref().storage, &non_existent);
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_new_hydromancer() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let hydromancer_address = make_valid_addr("hydromancer1");
        let result = insert_new_hydromancer(
            deps.as_mut().storage,
            hydromancer_address.clone(),
            "Test Hydromancer".to_string(),
            Decimal::percent(10),
        );

        assert!(result.is_ok());
        let hydromancer_id = result.unwrap();
        assert_eq!(hydromancer_id, 0);

        // Test getting hydromancer by ID
        let hydromancer = get_hydromancer(deps.as_ref().storage, hydromancer_id);
        assert!(hydromancer.is_ok());
        let hydromancer = hydromancer.unwrap();
        assert_eq!(hydromancer.hydromancer_id, 0);
        assert_eq!(hydromancer.address, hydromancer_address);
        assert_eq!(hydromancer.name, "Test Hydromancer");
        assert_eq!(hydromancer.commission_rate, Decimal::percent(10));

        // Test getting hydromancer ID by address
        let retrieved_id =
            get_hydromancer_id_by_address(deps.as_ref().storage, hydromancer_address);
        assert!(retrieved_id.is_ok());
        assert_eq!(retrieved_id.unwrap(), hydromancer_id);
    }

    #[test]
    fn test_hydromancer_exists() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        assert!(hydromancer_exists(deps.as_ref().storage, hydromancer_id).unwrap());
        assert!(!hydromancer_exists(deps.as_ref().storage, 999).unwrap());
    }

    #[test]
    fn test_get_all_hydromancers() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let id1 = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "H1".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let id2 = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer2"),
            "H2".to_string(),
            Decimal::percent(10),
        )
        .unwrap();

        let all_hydromancers = get_all_hydromancers(deps.as_ref().storage).unwrap();
        assert_eq!(all_hydromancers.len(), 2);
        assert!(all_hydromancers.contains(&id1));
        assert!(all_hydromancers.contains(&id2));
    }

    #[test]
    fn test_add_vessel() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user_address = make_valid_addr("user1");
        let user_id = insert_new_user(deps.as_mut().storage, user_address.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: Some(100),
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user_id,
        };

        let result = add_vessel(deps.as_mut().storage, &vessel, &user_address);
        assert!(result.is_ok());

        // Test vessel exists
        assert!(vessel_exists(deps.as_ref().storage, 1));

        // Test get vessel
        let retrieved_vessel = get_vessel(deps.as_ref().storage, 1);
        assert!(retrieved_vessel.is_ok());
        let retrieved = retrieved_vessel.unwrap();
        assert_eq!(retrieved.hydro_lock_id, 1);
        assert_eq!(retrieved.tokenized_share_record_id, Some(100));
        assert_eq!(retrieved.class_period, 1_000_000);
        assert_eq!(retrieved.auto_maintenance, true);
        assert_eq!(retrieved.hydromancer_id, Some(hydromancer_id));
        assert_eq!(retrieved.owner_id, user_id);

        // Test tokenized share record is used
        assert!(is_tokenized_share_record_used(deps.as_ref().storage, 100));
        assert!(!is_tokenized_share_record_used(deps.as_ref().storage, 999));
    }

    #[test]
    fn test_vessel_ownership() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();

        let vessel1 = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user1_id,
        };

        let vessel2 = Vessel {
            hydro_lock_id: 2,
            tokenized_share_record_id: None,
            class_period: 2_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user2_id,
        };

        add_vessel(deps.as_mut().storage, &vessel1, &user1).unwrap();
        add_vessel(deps.as_mut().storage, &vessel2, &user2).unwrap();

        // Test single vessel ownership
        assert!(is_vessel_owned_by(deps.as_ref().storage, &user1, 1).unwrap());
        assert!(!is_vessel_owned_by(deps.as_ref().storage, &user1, 2).unwrap());
        assert!(is_vessel_owned_by(deps.as_ref().storage, &user2, 2).unwrap());

        // Test multiple vessel ownership
        assert!(are_vessels_owned_by(deps.as_ref().storage, &user1, &[1]).unwrap());
        assert!(!are_vessels_owned_by(deps.as_ref().storage, &user1, &[1, 2]).unwrap());
        assert!(are_vessels_owned_by(deps.as_ref().storage, &user2, &[2]).unwrap());
    }

    #[test]
    fn test_get_vessels_by_owner() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        // Add multiple vessels for user1
        for i in 1..=5 {
            let vessel = Vessel {
                hydro_lock_id: i,
                tokenized_share_record_id: None,
                class_period: i as u64 * 1_000_000,
                auto_maintenance: false,
                hydromancer_id: None,
                owner_id: user1_id,
            };
            add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();
        }

        // Test getting all vessels
        let vessels = get_vessels_by_owner(deps.as_ref().storage, user1.clone(), 0, 10);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 5);

        // Test pagination
        let vessels = get_vessels_by_owner(deps.as_ref().storage, user1.clone(), 2, 2);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 2);

        // Test with non-existent user
        let non_user = make_valid_addr("non_user");
        let vessels = get_vessels_by_owner(deps.as_ref().storage, non_user, 0, 10);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 0);
    }

    #[test]
    fn test_get_vessels_by_hydromancer() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        // Add vessels controlled by hydromancer
        for i in 1..=3 {
            let vessel = Vessel {
                hydro_lock_id: i,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: false,
                hydromancer_id: Some(hydromancer_id),
                owner_id: user1_id,
            };
            add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();
        }

        let vessels = get_vessels_by_hydromancer(deps.as_ref().storage, hydromancer_id, 0, 10);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 3);

        // Test with non-existent hydromancer
        let vessels = get_vessels_by_hydromancer(deps.as_ref().storage, 999, 0, 10);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 0);
    }

    #[test]
    fn test_hydromancer_vessel_control() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let vessel1 = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };

        let vessel2 = Vessel {
            hydro_lock_id: 2,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None, // Under user control
            owner_id: user1_id,
        };

        add_vessel(deps.as_mut().storage, &vessel1, &user1).unwrap();
        add_vessel(deps.as_mut().storage, &vessel2, &user1).unwrap();

        // Test hydromancer control
        assert!(
            are_vessels_controlled_by_hydromancer(deps.as_ref().storage, hydromancer_id, &[1])
                .unwrap()
        );
        assert!(!are_vessels_controlled_by_hydromancer(
            deps.as_ref().storage,
            hydromancer_id,
            &[2]
        )
        .unwrap());
        assert!(!are_vessels_controlled_by_hydromancer(
            deps.as_ref().storage,
            hydromancer_id,
            &[1, 2]
        )
        .unwrap());

        // Test extracting non-controlled vessels
        let not_controlled = extract_vessels_not_controlled_by_hydromancer(
            deps.as_ref().storage,
            hydromancer_id,
            &[1, 2],
        );
        assert!(not_controlled.is_ok());
        let not_controlled = not_controlled.unwrap();
        assert_eq!(not_controlled, vec![2]);
    }

    #[test]
    fn test_vessel_harbor_operations() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        let vessel_harbor = VesselHarbor {
            hydro_lock_id: 1,
            steerer_id: user1_id,
            user_control: true,
        };

        // Test adding vessel to harbor
        let result = add_vessel_to_harbor(
            deps.as_mut().storage,
            1, // tranche_id
            1, // round_id
            1, // proposal_id
            &vessel_harbor,
        );
        assert!(result.is_ok());

        // Test getting vessel harbor
        let harbor_info = get_vessel_harbor(deps.as_ref().storage, 1, 1, 1);
        assert!(harbor_info.is_ok());
        let (harbor, proposal_id) = harbor_info.unwrap();
        assert_eq!(harbor.hydro_lock_id, 1);
        assert_eq!(harbor.steerer_id, user1_id);
        assert_eq!(harbor.user_control, true);
        assert_eq!(proposal_id, 1);

        // Test getting harbor of vessel
        let harbor_of_vessel = get_harbor_of_vessel(deps.as_ref().storage, 1, 1, 1);
        assert!(harbor_of_vessel.is_ok());
        assert_eq!(harbor_of_vessel.unwrap(), Some(1));

        // Test vessel is under user control
        assert!(is_vessel_used_under_user_control(
            deps.as_ref().storage,
            1,
            1,
            1
        ));

        // Test getting vessels by harbor ID
        let vessels_in_harbor = get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1);
        assert!(vessels_in_harbor.is_ok());
        let vessels = vessels_in_harbor.unwrap();
        assert_eq!(vessels.len(), 1);
        assert_eq!(vessels[0].0, 1); // vessel_id
        assert_eq!(vessels[0].1.hydro_lock_id, 1);

        // Test removing vessel from harbor
        let result = remove_vessel_harbor(deps.as_mut().storage, 1, 1, 1, 1);
        assert!(result.is_ok());

        // Verify vessel is no longer in harbor
        let harbor_info = get_vessel_harbor(deps.as_ref().storage, 1, 1, 1);
        assert!(harbor_info.is_err());

        assert!(!is_vessel_used_under_user_control(
            deps.as_ref().storage,
            1,
            1,
            1
        ));
    }

    #[test]
    fn test_vessel_shares_info() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let vessel_id = 1;
        let round_id = 1;
        let time_weighted_shares = 1000u128;
        let token_group_id = "test_token".to_string();
        let locked_rounds = 5u64;

        // Test saving vessel shares info
        let result = save_vessel_info_snapshot(
            deps.as_mut().storage,
            vessel_id,
            round_id,
            time_weighted_shares,
            token_group_id.clone(),
            locked_rounds,
            None,
        );
        assert!(result.is_ok());

        // Test has vessel shares info
        assert!(has_vessel_shares_info(
            deps.as_ref().storage,
            round_id,
            vessel_id
        ));
        assert!(!has_vessel_shares_info(
            deps.as_ref().storage,
            round_id,
            999
        ));

        // Test getting vessel shares info
        let shares_info = get_vessel_shares_info(deps.as_ref().storage, round_id, vessel_id);
        assert!(shares_info.is_ok());
        let info = shares_info.unwrap();
        assert_eq!(info.time_weighted_shares, time_weighted_shares);
        assert_eq!(info.token_group_id, token_group_id);
        assert_eq!(info.locked_rounds, locked_rounds);

        // Test getting non-existent shares info
        let non_existent = get_vessel_shares_info(deps.as_ref().storage, 999, vessel_id);
        assert!(non_existent.is_err());
    }

    #[test]
    fn test_auto_maintenance() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        // Enable auto maintenance
        let result = modify_auto_maintenance(deps.as_mut().storage, 1, true);
        assert!(result.is_ok());

        let updated_vessel = get_vessel(deps.as_ref().storage, 1).unwrap();
        assert_eq!(updated_vessel.auto_maintenance, true);

        // Test getting auto maintained vessel IDs by class
        let auto_maintained_map = get_vessel_ids_auto_maintained_by_class();
        assert!(auto_maintained_map.is_ok());

        // Disable auto maintenance
        let result = modify_auto_maintenance(deps.as_mut().storage, 1, false);
        assert!(result.is_ok());

        let updated_vessel = get_vessel(deps.as_ref().storage, 1).unwrap();
        assert_eq!(updated_vessel.auto_maintenance, false);

        // Test no change when setting same value
        let result = modify_auto_maintenance(deps.as_mut().storage, 1, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_vessel() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: Some(100),
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        // Verify vessel exists
        assert!(vessel_exists(deps.as_ref().storage, 1));
        assert!(is_tokenized_share_record_used(deps.as_ref().storage, 100));

        // Remove vessel
        let result = remove_vessel(deps.as_mut().storage, &user1, 1);
        assert!(result.is_ok());

        // Verify vessel is removed
        assert!(!vessel_exists(deps.as_ref().storage, 1));
        assert!(!is_tokenized_share_record_used(deps.as_ref().storage, 100));

        // Test removing non-existent vessel
        let result = remove_vessel(deps.as_mut().storage, &user1, 999);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_vessels_by_ids() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        // Add multiple vessels
        for i in 1..=3 {
            let vessel = Vessel {
                hydro_lock_id: i,
                tokenized_share_record_id: None,
                class_period: i as u64 * 1_000_000,
                auto_maintenance: false,
                hydromancer_id: None,
                owner_id: user1_id,
            };
            add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();
        }

        // Test getting multiple vessels by IDs
        let vessels = get_vessels_by_ids(deps.as_ref().storage, &[1, 3]);
        assert!(vessels.is_ok());
        let vessels = vessels.unwrap();
        assert_eq!(vessels.len(), 2);
        assert_eq!(vessels[0].hydro_lock_id, 1);
        assert_eq!(vessels[1].hydro_lock_id, 3);

        // Test getting non-existent vessel
        let vessels = get_vessels_by_ids(deps.as_ref().storage, &[999]);
        assert!(vessels.is_err());
    }

    #[test]
    fn test_change_vessel_hydromancer() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        let hydromancer1_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "H1".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let hydromancer2_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer2"),
            "H2".to_string(),
            Decimal::percent(10),
        )
        .unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: Some(hydromancer1_id),
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        // Test changing hydromancer
        let result = change_vessel_hydromancer(
            deps.as_mut().storage,
            1, // tranche_id
            1, // vessel_id
            1, // round_id
            hydromancer2_id,
        );
        assert!(result.is_ok());

        let updated_vessel = get_vessel(deps.as_ref().storage, 1).unwrap();
        assert_eq!(updated_vessel.hydromancer_id, Some(hydromancer2_id));

        // Test changing to same hydromancer (should be no-op)
        let result = change_vessel_hydromancer(
            deps.as_mut().storage,
            1, // tranche_id
            1, // vessel_id
            1, // round_id
            hydromancer2_id,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_vessel_hydromancer_management() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user1_id,
        };

        // Save vessel
        let result = save_vessel(deps.as_mut().storage, 1, &vessel);
        assert!(result.is_ok());

        // Add vessel to hydromancer
        let result = add_vessel_to_hydromancer(deps.as_mut().storage, hydromancer_id, 1);
        assert!(result.is_ok());

        // Verify vessel is controlled by hydromancer
        assert!(
            are_vessels_controlled_by_hydromancer(deps.as_ref().storage, hydromancer_id, &[1])
                .unwrap()
        );

        // Remove vessel from hydromancer
        let result = remove_vessel_from_hydromancer(deps.as_mut().storage, hydromancer_id, 1);
        assert!(result.is_ok());

        // Verify vessel is no longer controlled by hydromancer
        assert!(!are_vessels_controlled_by_hydromancer(
            deps.as_ref().storage,
            hydromancer_id,
            &[1]
        )
        .unwrap());
    }

    #[test]
    fn test_iterate_vessels_with_predicate() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        // Add vessels with different auto_maintenance settings
        for i in 1..=5 {
            let vessel = Vessel {
                hydro_lock_id: i,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: i % 2 == 0, // Even IDs have auto_maintenance
                hydromancer_id: None,
                owner_id: user1_id,
            };
            add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();
        }

        // Test filtering by auto_maintenance
        let auto_maintenance_vessels =
            iterate_vessels_with_predicate(deps.as_ref().storage, None, 10, |vessel| {
                vessel.auto_maintenance
            });

        assert!(auto_maintenance_vessels.is_ok());
        let vessels = auto_maintenance_vessels.unwrap();
        assert_eq!(vessels.len(), 2); // Vessels 2 and 4
        assert_eq!(vessels[0].0, 2);
        assert_eq!(vessels[1].0, 4);

        // Test with limit
        let limited_vessels =
            iterate_vessels_with_predicate(deps.as_ref().storage, None, 1, |vessel| {
                vessel.auto_maintenance
            });

        assert!(limited_vessels.is_ok());
        let vessels = limited_vessels.unwrap();
        assert_eq!(vessels.len(), 1);
        assert_eq!(vessels[0].0, 2);

        // Test with start_from_vessel_id
        let start_from_vessels =
            iterate_vessels_with_predicate(deps.as_ref().storage, Some(2), 10, |vessel| {
                vessel.auto_maintenance
            });

        assert!(start_from_vessels.is_ok());
        let vessels = start_from_vessels.unwrap();
        assert_eq!(vessels.len(), 1); // Only vessel 4 after vessel 2
        assert_eq!(vessels[0].0, 4);
    }

    #[test]
    fn test_time_weighted_shares_hydromancer() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let round_id = 1;
        let token_group_id = "test_token";
        let locked_rounds = 5;
        let shares = 1000u128;

        // Test adding time weighted shares
        let result = add_time_weighted_shares_to_hydromancer(
            deps.as_mut().storage,
            hydromancer_id,
            round_id,
            token_group_id,
            locked_rounds,
            shares,
        );
        assert!(result.is_ok());

        // Test getting time weighted shares
        let tws = get_hydromancer_time_weighted_shares_by_round(
            deps.as_ref().storage,
            round_id,
            hydromancer_id,
        );
        assert!(tws.is_ok());
        let tws = tws.unwrap();
        assert_eq!(tws.len(), 1);
        assert_eq!(tws[0].0 .0, locked_rounds);
        assert_eq!(tws[0].0 .1, token_group_id);
        assert_eq!(tws[0].1, shares);

        // Test adding more shares
        let result = add_time_weighted_shares_to_hydromancer(
            deps.as_mut().storage,
            hydromancer_id,
            round_id,
            token_group_id,
            locked_rounds,
            500u128,
        );
        assert!(result.is_ok());

        // Verify shares are accumulated
        let tws = get_hydromancer_time_weighted_shares_by_round(
            deps.as_ref().storage,
            round_id,
            hydromancer_id,
        );
        assert!(tws.is_ok());
        let tws = tws.unwrap();
        assert_eq!(tws[0].1, 1500u128);

        // Test subtracting shares
        let result = substract_time_weighted_shares_from_hydromancer(
            deps.as_mut().storage,
            hydromancer_id,
            round_id,
            token_group_id,
            locked_rounds,
            500u128,
        );
        assert!(result.is_ok());

        // Verify shares are reduced
        let tws = get_hydromancer_time_weighted_shares_by_round(
            deps.as_ref().storage,
            round_id,
            hydromancer_id,
        );
        assert!(tws.is_ok());
        let tws = tws.unwrap();
        assert_eq!(tws[0].1, 1000u128);
    }

    #[test]
    fn test_proposal_time_weighted_shares() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let proposal_id = 1;
        let token_group_id = "test_token";
        let shares = 1000u128;
        let current_round_id = 1;

        // Test adding proposal shares
        let result = add_time_weighted_shares_to_proposal(
            deps.as_mut().storage,
            current_round_id,
            proposal_id,
            token_group_id,
            shares,
        );
        assert!(result.is_ok());

        // Test getting proposal shares
        let proposal_tws =
            get_proposal_time_weighted_shares(deps.as_ref().storage, current_round_id, proposal_id);
        assert!(proposal_tws.is_ok());
        let tws = proposal_tws.unwrap();
        assert_eq!(tws.len(), 1);
        assert_eq!(tws[0].0, token_group_id);
        assert_eq!(tws[0].1, shares);

        // Test subtracting proposal shares
        let result = substract_time_weighted_shares_from_proposal(
            deps.as_mut().storage,
            current_round_id,
            proposal_id,
            token_group_id,
            500u128,
        );
        assert!(result.is_ok());

        // Verify shares are reduced
        let proposal_tws =
            get_proposal_time_weighted_shares(deps.as_ref().storage, current_round_id, proposal_id);
        assert!(proposal_tws.is_ok());
        let tws = proposal_tws.unwrap();
        assert_eq!(tws[0].1, 500u128);
    }

    #[test]
    fn test_hydromancer_proposal_time_weighted_shares() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let proposal_id = 1;
        let token_group_id = "test_token";
        let shares = 1000u128;

        // Test adding hydromancer proposal shares
        let result = add_time_weighted_shares_to_proposal_for_hydromancer(
            deps.as_mut().storage,
            proposal_id,
            hydromancer_id,
            token_group_id,
            shares,
        );
        assert!(result.is_ok());

        // Test getting hydromancer proposal shares
        let hp_tws = get_hydromancer_proposal_time_weighted_shares(
            deps.as_ref().storage,
            proposal_id,
            hydromancer_id,
        );
        assert!(hp_tws.is_ok());
        let tws = hp_tws.unwrap();
        assert_eq!(tws.len(), 1);
        assert_eq!(tws[0].0, token_group_id);
        assert_eq!(tws[0].1, shares);

        // Test subtracting hydromancer proposal shares
        let result = substract_time_weighted_shares_from_proposal_for_hydromancer(
            deps.as_mut().storage,
            proposal_id,
            hydromancer_id,
            token_group_id,
            300u128,
        );
        assert!(result.is_ok());

        // Verify shares are reduced
        let hp_tws = get_hydromancer_proposal_time_weighted_shares(
            deps.as_ref().storage,
            proposal_id,
            hydromancer_id,
        );
        assert!(hp_tws.is_ok());
        let tws = hp_tws.unwrap();
        assert_eq!(tws[0].1, 700u128);
    }

    #[test]
    fn test_take_control_of_vessels() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        // Verify vessel is under hydromancer control
        let vessel = get_vessel(deps.as_ref().storage, 1).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer_id));

        // Take control of vessel
        let result = take_control_of_vessels(deps.as_mut().storage, 1);
        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = get_vessel(deps.as_ref().storage, 1).unwrap();
        assert_eq!(vessel.hydromancer_id, None);
    }

    #[test]
    fn test_hydromancer_tws_completion_tracking() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        let hydromancer_id = insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Test".to_string(),
            Decimal::percent(5),
        )
        .unwrap();

        let round_id = 1;

        // Initially should not be complete
        assert!(!is_hydromancer_tws_complete(
            deps.as_ref().storage,
            round_id,
            hydromancer_id
        ));

        // Mark as complete
        let result = mark_hydromancer_tws_complete(deps.as_mut().storage, round_id, hydromancer_id);
        assert!(result.is_ok());

        // Should now be complete
        assert!(is_hydromancer_tws_complete(
            deps.as_ref().storage,
            round_id,
            hydromancer_id
        ));

        // Other round should not be complete
        assert!(!is_hydromancer_tws_complete(
            deps.as_ref().storage,
            2,
            hydromancer_id
        ));
    }

    #[test]
    fn test_error_conditions() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        // Test getting non-existent user
        let non_existent_user = make_valid_addr("non_existent");
        let result = get_user_id_by_address(deps.as_ref().storage, non_existent_user);
        assert!(result.is_err());

        // Test getting non-existent hydromancer
        let result = get_hydromancer(deps.as_ref().storage, 999);
        assert!(result.is_err());

        // Test getting non-existent vessel
        let result = get_vessel(deps.as_ref().storage, 999);
        assert!(result.is_err());

        // Test getting non-existent constants (should work with setup_basic_state)
        let result = get_constants(deps.as_ref().storage);
        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_cases_and_boundary_conditions() {
        let mut deps = mock_dependencies();
        setup_basic_state(deps.as_mut().storage);

        // Test with empty vessel lists
        let empty_vessels =
            get_vessels_by_owner(deps.as_ref().storage, make_valid_addr("empty"), 0, 10);
        assert!(empty_vessels.is_ok());
        assert_eq!(empty_vessels.unwrap().len(), 0);

        // Test with zero limit pagination
        let user1 = make_valid_addr("user1");
        let user1_id = insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();

        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: false,
            hydromancer_id: None,
            owner_id: user1_id,
        };
        add_vessel(deps.as_mut().storage, &vessel, &user1).unwrap();

        let vessels = get_vessels_by_owner(deps.as_ref().storage, user1, 0, 0);
        assert!(vessels.is_ok());
        assert_eq!(vessels.unwrap().len(), 0);

        // Test with very large start_index
        let vessels = get_vessels_by_owner(
            deps.as_ref().storage,
            make_valid_addr("user1"),
            usize::MAX,
            10,
        );
        assert!(vessels.is_ok());
        assert_eq!(vessels.unwrap().len(), 0);
    }
}
