#[cfg(test)]
mod tests {
    use zephyrus_core::state::Vessel;

    use crate::{
        helpers::auto_maintenance::{
            check_has_more_vessels_needing_maintenance, collect_vessels_needing_auto_maintenance,
            group_vessels_by_class_period, vessel_needs_auto_maintenance,
        },
        state,
        testing::make_valid_addr,
        testing_mocks::mock_dependencies,
    };
    use cosmwasm_std::{testing::mock_env, MessageInfo};
    use zephyrus_core::msgs::InstantiateMsg;

    fn init_contract(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) {
        use crate::contract::instantiate;
        let _ = instantiate(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("deployer"),
                funds: vec![],
            },
            InstantiateMsg {
                hydro_contract_address: make_valid_addr("hydro").into_string(),
                tribute_contract_address: make_valid_addr("tribute").into_string(),
                whitelist_admins: vec![make_valid_addr("admin").into_string()],
                default_hydromancer_name: make_valid_addr("zephyrus").into_string(),
                default_hydromancer_commission_rate: "0.1".parse().unwrap(),
                default_hydromancer_address: make_valid_addr("zephyrus").into_string(),
                commission_rate: "0.1".parse().unwrap(),
                commission_recipient: make_valid_addr("commission_recipient").into_string(),
            },
        );
    }

    fn setup_test_data(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) {
        init_contract(deps);

        // Add test users
        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");
        let user3 = make_valid_addr("user3");

        let user1_id = state::insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = state::insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();
        let user3_id = state::insert_new_user(deps.as_mut().storage, user3.clone()).unwrap();

        // Add test vessels with auto maintenance enabled
        // Vessel 0: target 1_000_000, auto maintenance enabled
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 0,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(0),
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        // Vessel 1: target 2_000_000, auto maintenance enabled
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: None,
                class_period: 2_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(0),
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();

        // Vessel 2: target 1_000_000, auto maintenance disabled
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 2,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: false,
                hydromancer_id: Some(0),
                owner_id: user3_id,
            },
            &user3,
        )
        .unwrap();

        // Vessel 3: target 3_000_000, auto maintenance enabled
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 3,
                tokenized_share_record_id: None,
                class_period: 3_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(0),
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        // Vessel 4: target 2_000_000, auto maintenance enabled
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 4,
                tokenized_share_record_id: None,
                class_period: 2_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(0),
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();
    }

    #[test]
    fn test_group_vessels_by_class_period() {
        let vessels = vec![
            (0, 1_000_000),
            (1, 2_000_000),
            (2, 1_000_000),
            (3, 3_000_000),
            (4, 2_000_000),
        ];

        let grouped = group_vessels_by_class_period(vessels);

        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped.get(&1_000_000).unwrap(), &vec![0, 2]);
        assert_eq!(grouped.get(&2_000_000).unwrap(), &vec![1, 4]);
        assert_eq!(grouped.get(&3_000_000).unwrap(), &vec![3]);
    }

    #[test]
    fn test_group_vessels_by_class_period_empty() {
        let vessels = vec![];
        let grouped = group_vessels_by_class_period(vessels);
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_group_vessels_by_class_period_single_class() {
        let vessels = vec![(0, 1_000_000), (1, 1_000_000), (2, 1_000_000)];

        let grouped = group_vessels_by_class_period(vessels);

        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped.get(&1_000_000).unwrap(), &vec![0, 1, 2]);
    }

    #[test]
    fn test_vessel_needs_auto_maintenance_no_shares() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let vessel_id = 0;
        let target_class_period = 1_000_000;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // No shares exist for this round - should need maintenance
        let needs_maintenance = vessel_needs_auto_maintenance(
            deps.as_ref().storage,
            vessel_id,
            target_class_period,
            current_round_id,
            lock_epoch_length,
        );

        assert!(needs_maintenance);
    }

    #[test]
    fn test_vessel_no_need_auto_maintenance_with_matching_shares() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let vessel_id = 0;
        let target_class_period = 1_000_000;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks
                                           // Add vessel shares with matching class period
                                           // Since locked_rounds * lock_epoch_length should equal target_class_period
                                           // We need locked_rounds = target_class_period / lock_epoch_length = 1_000_000 / 1_000_000 = 1
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            1000,
            "dAtom".to_string(),
            1, // locked_rounds = 1, so 1 * 1_000_000 = 1_000_000 (matches target)
            Some(0),
        )
        .unwrap();

        let needs_maintenance = vessel_needs_auto_maintenance(
            deps.as_ref().storage,
            vessel_id,
            target_class_period,
            current_round_id,
            lock_epoch_length,
        );

        assert!(!needs_maintenance);
    }

    #[test]
    fn test_vessel_needs_auto_maintenance_with_mismatched_locked_rounds() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let vessel_id = 0;
        let target_class_period = 1_000_000;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks
        let constants = state::get_constants(deps.as_ref().storage).unwrap();
        // Add vessel shares with different locked_rounds
        // locked_rounds = 2, so 2 * 1_000_000 = 2_000_000 (does not match target 1_000_000)
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2, // Different from target
            Some(0),
        )
        .unwrap();

        let needs_maintenance = vessel_needs_auto_maintenance(
            deps.as_ref().storage,
            vessel_id,
            target_class_period,
            current_round_id,
            lock_epoch_length,
        );

        assert!(needs_maintenance);
    }

    #[test]
    fn test_collect_vessels_needing_auto_maintenance_all_need_maintenance() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let limit = 10;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // No shares exist for any vessel - all auto-maintained vessels should need maintenance
        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            limit,
            lock_epoch_length,
        )
        .unwrap();

        // Should return vessels 0, 1, 3, 4 (auto maintenance enabled)
        // Vessel 2 has auto maintenance disabled, so not included
        assert_eq!(vessels.len(), 4);

        // Check that vessels are sorted by ID
        assert_eq!(vessels[0], (0, 1_000_000));
        assert_eq!(vessels[1], (1, 2_000_000));
        assert_eq!(vessels[2], (3, 3_000_000));
        assert_eq!(vessels[3], (4, 2_000_000));
    }

    #[test]
    fn test_collect_vessels_needing_auto_maintenance_with_pagination() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let limit = 2;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // First page
        let vessels_page1 = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            limit,
            lock_epoch_length,
        )
        .unwrap();

        assert_eq!(vessels_page1.len(), 2);
        assert_eq!(vessels_page1[0], (0, 1_000_000));
        assert_eq!(vessels_page1[1], (1, 2_000_000));

        // Second page starting from vessel 1
        let vessels_page2 = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            Some(1),
            limit,
            lock_epoch_length,
        )
        .unwrap();

        assert_eq!(vessels_page2.len(), 2);
        assert_eq!(vessels_page2[0], (3, 3_000_000));
        assert_eq!(vessels_page2[1], (4, 2_000_000));
    }

    #[test]
    fn test_collect_vessels_needing_auto_maintenance_some_have_correct_shares() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let limit = 10;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // Add correct shares for vessel 0 and vessel 1
        // For vessel 0: target 1_000_000, locked_rounds should be 1 (1 * 1_000_000 = 1_000_000)
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            0,
            current_round_id,
            1000,
            "dAtom".to_string(),
            1, // locked_rounds = 1
            Some(0),
        )
        .unwrap();

        // For vessel 1: target 2_000_000, locked_rounds should be 2 (2 * 1_000_000 = 2_000_000)
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            1,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2, // locked_rounds = 2
            Some(0),
        )
        .unwrap();

        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            limit,
            lock_epoch_length,
        )
        .unwrap();

        // Should only return vessels 3 and 4 (which don't have shares)
        assert_eq!(vessels.len(), 2);
        assert_eq!(vessels[0], (3, 3_000_000));
        assert_eq!(vessels[1], (4, 2_000_000));
    }

    #[test]
    fn test_collect_vessels_needing_auto_maintenance_empty_result() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let limit = 10;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // Add correct shares for all auto-maintained vessels
        let vessels_to_setup = vec![
            (0, 1_000_000, 1), // locked_rounds = 1
            (1, 2_000_000, 2), // locked_rounds = 2
            (3, 3_000_000, 3), // locked_rounds = 3
            (4, 2_000_000, 2), // locked_rounds = 2
        ];

        for (vessel_id, _class_period, locked_rounds) in vessels_to_setup {
            state::save_vessel_info_snapshot(
                deps.as_mut().storage,
                vessel_id,
                current_round_id,
                1000,
                "dAtom".to_string(),
                locked_rounds,
                Some(0),
            )
            .unwrap();
        }

        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            limit,
            lock_epoch_length,
        )
        .unwrap();

        assert_eq!(vessels.len(), 0);
    }

    #[test]
    fn test_check_has_more_vessels_needing_maintenance_true() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let last_processed_vessel_id = 1; // Vessels 3 and 4 come after this
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        let has_more = check_has_more_vessels_needing_maintenance(
            deps.as_ref().storage,
            current_round_id,
            last_processed_vessel_id,
            lock_epoch_length,
        )
        .unwrap();

        assert!(has_more);
    }

    #[test]
    fn test_check_has_more_vessels_needing_maintenance_false() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let last_processed_vessel_id = 4; // No vessels after this
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        let has_more = check_has_more_vessels_needing_maintenance(
            deps.as_ref().storage,
            current_round_id,
            last_processed_vessel_id,
            lock_epoch_length,
        )
        .unwrap();

        assert!(!has_more);
    }

    #[test]
    fn test_check_has_more_vessels_needing_maintenance_with_correct_shares() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let last_processed_vessel_id = 1;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // Add correct shares for vessels 3 and 4
        // For vessel 3: target 3_000_000, locked_rounds should be 3 (3 * 1_000_000 = 3_000_000)
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            3,
            current_round_id,
            1000,
            "dAtom".to_string(),
            3, // locked_rounds = 3
            Some(0),
        )
        .unwrap();

        // For vessel 4: target 2_000_000, locked_rounds should be 2 (2 * 1_000_000 = 2_000_000)
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            4,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2, // locked_rounds = 2
            Some(0),
        )
        .unwrap();

        let has_more = check_has_more_vessels_needing_maintenance(
            deps.as_ref().storage,
            current_round_id,
            last_processed_vessel_id,
            lock_epoch_length,
        )
        .unwrap();

        assert!(!has_more);
    }

    #[test]
    fn test_pagination_boundary_conditions() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        // Test with limit 0
        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            0,
            lock_epoch_length,
        )
        .unwrap();
        assert_eq!(vessels.len(), 0);

        // Test with limit 1
        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            1,
            lock_epoch_length,
        )
        .unwrap();
        assert_eq!(vessels.len(), 1);
        assert_eq!(vessels[0], (0, 1_000_000));

        // Test starting from non-existent vessel ID
        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            Some(100), // Non-existent vessel ID
            10,
            lock_epoch_length,
        )
        .unwrap();
        assert_eq!(vessels.len(), 0);

        // Test starting from last vessel ID
        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            Some(4),
            10,
            lock_epoch_length,
        )
        .unwrap();
        assert_eq!(vessels.len(), 0);
    }

    #[test]
    fn test_collect_vessels_large_limit() {
        let mut deps = mock_dependencies();
        setup_test_data(&mut deps);

        let current_round_id = 1;
        let limit = 1000; // Very large limit
        let lock_epoch_length = 1_000_000; // Use the same as in testing_mocks

        let vessels = collect_vessels_needing_auto_maintenance(
            deps.as_ref().storage,
            current_round_id,
            None,
            limit,
            lock_epoch_length,
        )
        .unwrap();

        // Should still only return the 4 auto-maintained vessels
        assert_eq!(vessels.len(), 4);
    }
}
