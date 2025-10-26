#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_env, MessageInfo};
    use zephyrus_core::msgs::InstantiateMsg;
    use zephyrus_core::state::{Vessel, VesselHarbor};

    use crate::{
        helpers::vessel_assignment::{
            assign_vessel_to_hydromancer, assign_vessel_to_user_control,
            categorize_vessels_by_control,
        },
        state,
        testing::make_valid_addr,
        testing_mocks::mock_dependencies,
    };

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
                min_tokens_per_vessel: 5_000_000,
            },
        );
    }

    fn setup_test_data(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) -> (u64, u64, u64) {
        init_contract(deps);

        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");
        let user3 = make_valid_addr("user3");

        let user1_id = state::insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = state::insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();
        let user3_id = state::insert_new_user(deps.as_mut().storage, user3.clone()).unwrap();

        // Create two hydromancers
        let hydromancer1_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer1"),
            "Hydromancer 1".to_string(),
            "0.1".parse().unwrap(),
        )
        .unwrap();

        let hydromancer2_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer2"),
            "Hydromancer 2".to_string(),
            "0.15".parse().unwrap(),
        )
        .unwrap();

        // Add vessels in different states
        // Vessel 1: Under hydromancer1 control
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(hydromancer1_id),
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        // Vessel 2: Under user control
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 2,
                tokenized_share_record_id: None,
                class_period: 2_000_000,
                auto_maintenance: false,
                hydromancer_id: None,
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();

        // Vessel 3: Under hydromancer2 control
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 3,
                tokenized_share_record_id: None,
                class_period: 3_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(hydromancer2_id),
                owner_id: user3_id,
            },
            &user3,
        )
        .unwrap();

        // Vessel 4: Under user control
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 4,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: false,
                hydromancer_id: None,
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        // Add vessels to hydromancer mappings
        state::add_vessel_to_hydromancer(deps.as_mut().storage, hydromancer1_id, 1).unwrap();
        state::add_vessel_to_hydromancer(deps.as_mut().storage, hydromancer2_id, 3).unwrap();

        (user1_id, hydromancer1_id, hydromancer2_id)
    }

    fn setup_vessel_with_tws(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
        vessel_id: u64,
        current_round_id: u64,
    ) {
        // Add vessel shares info to simulate TWS
        state::save_vessel_shares_info(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2,
        )
        .unwrap();

        // Get vessel to check if it has hydromancer
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();

        // If vessel has hydromancer, add TWS to hydromancer totals
        if let Some(hydromancer_id) = vessel.hydromancer_id {
            state::add_time_weighted_shares_to_hydromancer(
                deps.as_mut().storage,
                hydromancer_id,
                current_round_id,
                "dAtom",
                2,
                1000,
            )
            .unwrap();
        }
    }

    fn setup_vessel_in_proposal(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
        vessel_id: u64,
        tranche_id: u64,
        current_round_id: u64,
        proposal_id: u64,
    ) {
        // Get vessel to determine correct steerer_id and control state
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();

        // Add vessel to harbor
        let vessel_harbor = VesselHarbor {
            user_control: vessel.hydromancer_id.is_none(),
            steerer_id: vessel.hydromancer_id.unwrap_or(vessel.owner_id),
            hydro_lock_id: vessel_id,
        };

        state::add_vessel_to_harbor(
            deps.as_mut().storage,
            tranche_id,
            current_round_id,
            proposal_id,
            &vessel_harbor,
        )
        .unwrap();

        // Add TWS to proposal totals
        state::add_time_weighted_shares_to_proposal(
            deps.as_mut().storage,
            proposal_id,
            "dAtom",
            1000,
        )
        .unwrap();

        // Add TWS to hydromancer-specific proposal totals if applicable
        if let Some(hydromancer_id) = vessel.hydromancer_id {
            state::add_time_weighted_shares_to_proposal_for_hydromancer(
                deps.as_mut().storage,
                proposal_id,
                hydromancer_id,
                "dAtom",
                1000,
            )
            .unwrap();
        }
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_no_tws() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 2; // Currently under user control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer1_id));
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_already_assigned() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Already assigned to hydromancer1
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel assignment unchanged
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer1_id));
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_with_tws() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 2; // Currently under user control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        // Setup vessel with TWS
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer1_id));

        // Verify vessel shares info still exists
        let vessel_shares =
            state::get_vessel_shares_info(deps.as_ref().storage, current_round_id, vessel_id);
        assert!(vessel_shares.is_ok());
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_from_another_hydromancer() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer2_id) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        // Setup vessel with TWS
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer2_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer2
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer2_id));
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_with_proposal() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer2_id) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1];
        let proposal_id = 100;

        // Setup vessel with TWS and in proposal
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);
        setup_vessel_in_proposal(
            &mut deps,
            vessel_id,
            tranche_ids[0],
            current_round_id,
            proposal_id,
        );

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer2_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer2
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer2_id));

        // Verify vessel is no longer in the proposal
        let harbor = state::get_harbor_of_vessel(
            deps.as_ref().storage,
            tranche_ids[0],
            current_round_id,
            vessel_id,
        );
        assert!(harbor.unwrap().is_none());
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_nonexistent_vessel() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 999; // Non-existent vessel
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_assign_vessel_to_user_control_from_hydromancer() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);
    }

    #[test]
    fn test_assign_vessel_to_user_control_already_user_control() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 2; // Already under user control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is still under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);
    }

    #[test]
    fn test_assign_vessel_to_user_control_with_tws() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        // Setup vessel with TWS
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);

        // Verify vessel shares info still exists
        let vessel_shares =
            state::get_vessel_shares_info(deps.as_ref().storage, current_round_id, vessel_id);
        assert!(vessel_shares.is_ok());
    }

    #[test]
    fn test_assign_vessel_to_user_control_with_proposal() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1];
        let proposal_id = 100;

        // Setup vessel with TWS and in proposal
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);
        setup_vessel_in_proposal(
            &mut deps,
            vessel_id,
            tranche_ids[0],
            current_round_id,
            proposal_id,
        );

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);

        // Verify vessel is no longer in the proposal
        let harbor = state::get_harbor_of_vessel(
            deps.as_ref().storage,
            tranche_ids[0],
            current_round_id,
            vessel_id,
        );
        assert!(harbor.is_err() || harbor.unwrap().is_none());
    }

    #[test]
    fn test_assign_vessel_to_user_control_nonexistent_vessel() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 999; // Non-existent vessel
        let current_round_id = 1;
        let tranche_ids = vec![1];

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_categorize_vessels_by_control_all_not_controlled() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![2, 4]; // Both under user control
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, vec![2, 4]);
        assert_eq!(already_controlled, Vec::<u64>::new());
    }

    #[test]
    fn test_categorize_vessels_by_control_all_controlled() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![1]; // Under hydromancer1 control
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, Vec::<u64>::new());
        assert_eq!(already_controlled, vec![1]);
    }

    #[test]
    fn test_categorize_vessels_by_control_mixed() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![1, 2, 3, 4]; // Mixed control
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, vec![2, 3, 4]); // 2,4 user control, 3 under different hydromancer
        assert_eq!(already_controlled, vec![1]);
    }

    #[test]
    fn test_categorize_vessels_by_control_empty_list() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![];
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, Vec::<u64>::new());
        assert_eq!(already_controlled, Vec::<u64>::new());
    }

    #[test]
    fn test_categorize_vessels_by_control_nonexistent_vessel() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![999]; // Non-existent vessel
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_err());
    }

    #[test]
    fn test_categorize_vessels_by_control_different_hydromancer() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![1, 3]; // 1 under hydromancer1, 3 under hydromancer2
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, vec![3]); // 3 under different hydromancer
        assert_eq!(already_controlled, vec![1]);
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_multiple_tranches() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer2_id) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1, 2, 3]; // Multiple tranches
        let proposal_id = 100;

        // Setup vessel with TWS and in proposal for multiple tranches
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);
        for &tranche_id in &tranche_ids {
            setup_vessel_in_proposal(
                &mut deps,
                vessel_id,
                tranche_id,
                current_round_id,
                proposal_id,
            );
        }

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer2_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer2
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer2_id));

        // Verify vessel is no longer in any proposal
        for &tranche_id in &tranche_ids {
            let harbor = state::get_harbor_of_vessel(
                deps.as_ref().storage,
                tranche_id,
                current_round_id,
                vessel_id,
            );
            assert!(harbor.is_err() || harbor.unwrap().is_none());
        }
    }

    #[test]
    fn test_assign_vessel_to_user_control_multiple_tranches() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Currently under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![1, 2, 3]; // Multiple tranches
        let proposal_id = 100;

        // Setup vessel with TWS and in proposal for multiple tranches
        setup_vessel_with_tws(&mut deps, vessel_id, current_round_id);
        for &tranche_id in &tranche_ids {
            setup_vessel_in_proposal(
                &mut deps,
                vessel_id,
                tranche_id,
                current_round_id,
                proposal_id,
            );
        }

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);

        // Verify vessel is no longer in any proposal
        for &tranche_id in &tranche_ids {
            let harbor = state::get_harbor_of_vessel(
                deps.as_ref().storage,
                tranche_id,
                current_round_id,
                vessel_id,
            );
            assert!(harbor.is_err() || harbor.unwrap().is_none());
        }
    }

    #[test]
    fn test_assign_vessel_to_hydromancer_edge_cases() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 2; // Under user control
        let current_round_id = 1;
        let tranche_ids = vec![]; // Empty tranche list

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now assigned to hydromancer
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, Some(hydromancer1_id));
    }

    #[test]
    fn test_assign_vessel_to_user_control_edge_cases() {
        let mut deps = mock_dependencies();
        let (_, _, _) = setup_test_data(&mut deps);

        let vessel_id = 1; // Under hydromancer1 control
        let current_round_id = 1;
        let tranche_ids = vec![]; // Empty tranche list

        let result = assign_vessel_to_user_control(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify vessel is now under user control
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(vessel.hydromancer_id, None);
    }

    #[test]
    fn test_categorize_vessels_by_control_single_vessel() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_ids = vec![1]; // Single vessel under control
        let result =
            categorize_vessels_by_control(deps.as_ref().storage, hydromancer1_id, &vessel_ids);

        assert!(result.is_ok());
        let (not_controlled, already_controlled) = result.unwrap();
        assert_eq!(not_controlled, Vec::<u64>::new());
        assert_eq!(already_controlled, vec![1]);
    }

    #[test]
    fn test_vessel_assignment_preserves_other_fields() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, _) = setup_test_data(&mut deps);

        let vessel_id = 2; // Under user control
        let current_round_id = 1;
        let tranche_ids = vec![1];

        // Get original vessel to verify other fields are preserved
        let original_vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();

        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            vessel_id,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );

        assert!(result.is_ok());

        // Verify only hydromancer_id changed
        let updated_vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        assert_eq!(updated_vessel.hydro_lock_id, original_vessel.hydro_lock_id);
        assert_eq!(
            updated_vessel.tokenized_share_record_id,
            original_vessel.tokenized_share_record_id
        );
        assert_eq!(updated_vessel.class_period, original_vessel.class_period);
        assert_eq!(
            updated_vessel.auto_maintenance,
            original_vessel.auto_maintenance
        );
        assert_eq!(updated_vessel.owner_id, original_vessel.owner_id);
        assert_eq!(updated_vessel.hydromancer_id, Some(hydromancer1_id));
    }

    #[test]
    fn test_multiple_vessel_assignments() {
        let mut deps = mock_dependencies();
        let (_, hydromancer1_id, hydromancer2_id) = setup_test_data(&mut deps);

        let current_round_id = 1;
        let tranche_ids = vec![1];

        // Assign vessel 2 to hydromancer1
        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            2,
            hydromancer1_id,
            current_round_id,
            &tranche_ids,
        );
        assert!(result.is_ok());

        // Assign vessel 4 to hydromancer2
        let result = assign_vessel_to_hydromancer(
            deps.as_mut().storage,
            4,
            hydromancer2_id,
            current_round_id,
            &tranche_ids,
        );
        assert!(result.is_ok());

        // Verify both assignments
        let vessel2 = state::get_vessel(deps.as_ref().storage, 2).unwrap();
        let vessel4 = state::get_vessel(deps.as_ref().storage, 4).unwrap();
        assert_eq!(vessel2.hydromancer_id, Some(hydromancer1_id));
        assert_eq!(vessel4.hydromancer_id, Some(hydromancer2_id));

        // Now move vessel 2 to user control
        let result =
            assign_vessel_to_user_control(deps.as_mut().storage, 2, current_round_id, &tranche_ids);
        assert!(result.is_ok());

        // Verify final state
        let vessel2_final = state::get_vessel(deps.as_ref().storage, 2).unwrap();
        let vessel4_final = state::get_vessel(deps.as_ref().storage, 4).unwrap();
        assert_eq!(vessel2_final.hydromancer_id, None);
        assert_eq!(vessel4_final.hydromancer_id, Some(hydromancer2_id));
    }
}
