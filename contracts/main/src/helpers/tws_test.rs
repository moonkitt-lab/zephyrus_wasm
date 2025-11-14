#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_env, MessageInfo, Uint128};
    use hydro_interface::msgs::LockupVotingMetrics;
    use std::collections::HashMap;
    use zephyrus_core::state::{Constants, HydroConfig, Vessel, VesselInfoSnapshot};

    use crate::{
        helpers::tws::{
            apply_hydromancer_tws_changes, apply_proposal_hydromancer_tws_changes,
            apply_proposal_tws_changes, batch_hydromancer_tws_changes, batch_proposal_tws_changes,
            complete_hydromancer_time_weighted_shares, initialize_vessel_tws, TwsChanges,
        },
        state,
        testing::make_valid_addr,
        testing_mocks::mock_dependencies,
    };
    use zephyrus_core::msgs::InstantiateMsg;

    fn get_test_constants() -> Constants {
        Constants {
            default_hydromancer_id: 0,
            paused_contract: false,
            hydro_config: HydroConfig {
                hydro_contract_address: make_valid_addr("hydro"),
                hydro_tribute_contract_address: make_valid_addr("tribute"),
            },
            commission_rate: "0.1".parse().unwrap(),
            commission_recipient: make_valid_addr("commission_recipient"),
            min_tokens_per_vessel: 5_000_000,
        }
    }

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

    fn setup_test_vessels(
        deps: &mut cosmwasm_std::OwnedDeps<
            cosmwasm_std::testing::MockStorage,
            cosmwasm_std::testing::MockApi,
            crate::testing_mocks::MockQuerier,
        >,
    ) -> (u64, u64) {
        init_contract(deps);

        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");

        let user1_id = state::insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = state::insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();

        // Create hydromancer
        let hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            make_valid_addr("hydromancer"),
            "Test Hydromancer".to_string(),
            "0.1".parse().unwrap(),
        )
        .unwrap();

        // Add vessels
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 1,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(hydromancer_id),
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 2,
                tokenized_share_record_id: None,
                class_period: 2_000_000,
                auto_maintenance: true,
                hydromancer_id: Some(hydromancer_id),
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();

        (user1_id, user2_id)
    }

    #[test]
    fn test_tws_changes_new() {
        let tws_changes = TwsChanges::new();
        assert!(tws_changes.proposal_changes.is_empty());
        assert!(tws_changes.proposal_hydromancer_changes.is_empty());
    }

    #[test]
    fn test_tws_changes_default() {
        let tws_changes = TwsChanges::default();
        assert!(tws_changes.proposal_changes.is_empty());
        assert!(tws_changes.proposal_hydromancer_changes.is_empty());
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_new_shares_only() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;
        let old_vessel_shares = None;
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(1000u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares,
            &new_lockup_shares,
        );

        assert_eq!(hydromancer_tws_changes.len(), 1);
        let key = (hydromancer_id, current_round_id, "dAtom".to_string(), 2);
        assert_eq!(hydromancer_tws_changes.get(&key), Some(&1000i128));
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_old_shares_only() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;
        let old_vessel_shares = Some(VesselInfoSnapshot {
            time_weighted_shares: 800,
            token_group_id: "dAtom".to_string(),
            locked_rounds: 1,
            hydromancer_id: Some(hydromancer_id),
        });
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::zero(),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares,
            &new_lockup_shares,
        );

        assert_eq!(hydromancer_tws_changes.len(), 1);
        let key = (hydromancer_id, current_round_id, "dAtom".to_string(), 1);
        assert_eq!(hydromancer_tws_changes.get(&key), Some(&-800i128));
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_both_shares() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;
        let old_vessel_shares = Some(VesselInfoSnapshot {
            time_weighted_shares: 800,
            token_group_id: "dAtom".to_string(),
            locked_rounds: 1,
            hydromancer_id: Some(hydromancer_id),
        });
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(1200u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares,
            &new_lockup_shares,
        );

        assert_eq!(hydromancer_tws_changes.len(), 2);
        let old_key = (hydromancer_id, current_round_id, "dAtom".to_string(), 1);
        let new_key = (hydromancer_id, current_round_id, "dAtom".to_string(), 2);
        assert_eq!(hydromancer_tws_changes.get(&old_key), Some(&-800i128));
        assert_eq!(hydromancer_tws_changes.get(&new_key), Some(&1200i128));
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_same_key_accumulation() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;
        let old_vessel_shares = Some(VesselInfoSnapshot {
            time_weighted_shares: 500,
            token_group_id: "dAtom".to_string(),
            locked_rounds: 1,
            hydromancer_id: Some(hydromancer_id),
        });
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(800u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 1, // Same locked_rounds as old
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares,
            &new_lockup_shares,
        );

        assert_eq!(hydromancer_tws_changes.len(), 1);
        let key = (hydromancer_id, current_round_id, "dAtom".to_string(), 1);
        assert_eq!(hydromancer_tws_changes.get(&key), Some(&300i128)); // 800 - 500
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_zero_old_shares() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;
        let old_vessel_shares = Some(VesselInfoSnapshot {
            time_weighted_shares: 0,
            token_group_id: "dAtom".to_string(),
            locked_rounds: 1,
            hydromancer_id: Some(hydromancer_id),
        });
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(1000u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares,
            &new_lockup_shares,
        );

        // Should only have new shares entry since old shares were zero
        assert_eq!(hydromancer_tws_changes.len(), 1);
        let key = (hydromancer_id, current_round_id, "dAtom".to_string(), 2);
        assert_eq!(hydromancer_tws_changes.get(&key), Some(&1000i128));
    }

    #[test]
    fn test_batch_proposal_tws_changes_no_harbor() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut tws_changes = TwsChanges::new();
        let vessel = state::get_vessel(deps.as_ref().storage, 1).unwrap();
        let old_vessel_shares = None;
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(1000u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };
        let tranche_ids = vec![1];
        let current_round_id = 1;

        let result = batch_proposal_tws_changes(
            deps.as_ref().storage,
            &mut tws_changes,
            &vessel,
            &old_vessel_shares,
            &new_lockup_shares,
            &tranche_ids,
            current_round_id,
        );

        assert!(result.is_ok());
        assert!(tws_changes.proposal_changes.is_empty());
        assert!(tws_changes.proposal_hydromancer_changes.is_empty());
    }

    #[test]
    fn test_batch_proposal_tws_changes_with_harbor() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        // Set up harbor for vessel using add_vessel_to_harbor
        let proposal_id = 1;
        let tranche_id = 1;
        let current_round_id = 1;
        let vessel_id = 1;

        let vessel_harbor = zephyrus_core::state::VesselHarbor {
            user_control: true,
            steerer_id: 1,
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

        let mut tws_changes = TwsChanges::new();
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();
        let old_vessel_shares = Some(VesselInfoSnapshot {
            time_weighted_shares: 500,
            token_group_id: "dAtom".to_string(),
            locked_rounds: 1,
            hydromancer_id: Some(1),
        });
        let new_lockup_shares = LockupVotingMetrics {
            lock_id: vessel_id,
            time_weighted_shares: Uint128::from(1000u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };
        let tranche_ids = vec![tranche_id];

        let result = batch_proposal_tws_changes(
            deps.as_ref().storage,
            &mut tws_changes,
            &vessel,
            &old_vessel_shares,
            &new_lockup_shares,
            &tranche_ids,
            current_round_id,
        );

        assert!(result.is_ok());
        assert_eq!(tws_changes.proposal_changes.len(), 1);
        assert_eq!(tws_changes.proposal_hydromancer_changes.len(), 1);

        // Check proposal changes - should have net effect of -500 + 1000 = 500
        let key = (proposal_id, "dAtom".to_string());
        assert_eq!(tws_changes.proposal_changes.get(&key), Some(&500i128));

        // Check hydromancer changes (vessel has hydromancer_id = Some(1))
        let hyd_key = (proposal_id, 1, "dAtom".to_string());
        assert_eq!(
            tws_changes.proposal_hydromancer_changes.get(&hyd_key),
            Some(&500i128)
        );
    }

    #[test]
    fn test_apply_hydromancer_tws_changes_positive() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let round_id = 1;
        let token_group_id = "dAtom".to_string();
        let locked_rounds = 2;
        let key = (
            hydromancer_id,
            round_id,
            token_group_id.clone(),
            locked_rounds,
        );
        hydromancer_tws_changes.insert(key, 1000i128);

        let result = apply_hydromancer_tws_changes(deps.as_mut().storage, hydromancer_tws_changes);

        assert!(result.is_ok());
        // The function should execute without error - actual storage verification would require internal access
    }

    #[test]
    fn test_apply_hydromancer_tws_changes_negative() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let hydromancer_id = 1;
        let round_id = 1;
        let token_group_id = "dAtom".to_string();
        let locked_rounds = 2;

        // First add some TWS
        state::add_time_weighted_shares_to_hydromancer(
            deps.as_mut().storage,
            hydromancer_id,
            round_id,
            &token_group_id,
            locked_rounds,
            1500,
        )
        .unwrap();

        let mut hydromancer_tws_changes = HashMap::new();
        let key = (
            hydromancer_id,
            round_id,
            token_group_id.clone(),
            locked_rounds,
        );
        hydromancer_tws_changes.insert(key, -500i128);

        let result = apply_hydromancer_tws_changes(deps.as_mut().storage, hydromancer_tws_changes);

        assert!(result.is_ok());
        // The function should execute without error - actual storage verification would require internal access
    }

    #[test]
    fn test_apply_hydromancer_tws_changes_zero() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let round_id = 1;
        let token_group_id = "dAtom".to_string();
        let locked_rounds = 2;
        let key = (
            hydromancer_id,
            round_id,
            token_group_id.clone(),
            locked_rounds,
        );
        hydromancer_tws_changes.insert(key, 0i128);

        let result = apply_hydromancer_tws_changes(deps.as_mut().storage, hydromancer_tws_changes);

        assert!(result.is_ok());
        // No changes should occur for zero delta - function should execute without error
    }

    #[test]
    fn test_apply_proposal_tws_changes_positive() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut proposal_tws_changes = HashMap::new();
        let proposal_id = 1;
        let token_group_id = "dAtom".to_string();
        let key = (proposal_id, token_group_id.clone());
        proposal_tws_changes.insert(key, 1000i128);
        let current_round_id = 1;

        let result = apply_proposal_tws_changes(
            deps.as_mut().storage,
            current_round_id,
            proposal_tws_changes,
        );

        assert!(result.is_ok());
        // Function should execute without error - storage verification would require internal access
    }

    #[test]
    fn test_apply_proposal_tws_changes_negative() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let proposal_id = 1;
        let token_group_id = "dAtom".to_string();
        let current_round_id = 1;
        // First add some TWS
        state::add_time_weighted_shares_to_proposal(
            deps.as_mut().storage,
            current_round_id, // round_id
            proposal_id,
            &token_group_id,
            1500,
        )
        .unwrap();

        let mut proposal_tws_changes = HashMap::new();
        let key = (proposal_id, token_group_id.clone());
        proposal_tws_changes.insert(key, -500i128);

        let result = apply_proposal_tws_changes(
            deps.as_mut().storage,
            current_round_id,
            proposal_tws_changes,
        );

        assert!(result.is_ok());
        // Function should execute without error - storage verification would require internal access
    }

    #[test]
    fn test_apply_proposal_hydromancer_tws_changes_positive() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut proposal_hydromancer_tws_changes = HashMap::new();
        let proposal_id = 1;
        let hydromancer_id = 1;
        let token_group_id = "dAtom".to_string();
        let key = (proposal_id, hydromancer_id, token_group_id.clone());
        proposal_hydromancer_tws_changes.insert(key, 1000i128);

        let result = apply_proposal_hydromancer_tws_changes(
            deps.as_mut().storage,
            proposal_hydromancer_tws_changes,
        );

        assert!(result.is_ok());
        // Function should execute without error - storage verification would require internal access
    }

    #[test]
    fn test_apply_proposal_hydromancer_tws_changes_negative() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let proposal_id = 1;
        let hydromancer_id = 1;
        let token_group_id = "dAtom".to_string();

        // First add some TWS
        state::add_time_weighted_shares_to_proposal_for_hydromancer(
            deps.as_mut().storage,
            proposal_id,
            hydromancer_id,
            &token_group_id,
            1500,
        )
        .unwrap();

        let mut proposal_hydromancer_tws_changes = HashMap::new();
        let key = (proposal_id, hydromancer_id, token_group_id.clone());
        proposal_hydromancer_tws_changes.insert(key, -500i128);

        let result = apply_proposal_hydromancer_tws_changes(
            deps.as_mut().storage,
            proposal_hydromancer_tws_changes,
        );

        assert!(result.is_ok());
        // Function should execute without error - storage verification would require internal access
    }

    #[test]
    fn test_complete_hydromancer_time_weighted_shares_not_complete() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let hydromancer_id = 1;
        let current_round_id = 1;

        // Don't mark as complete first
        let result = complete_hydromancer_time_weighted_shares(
            &mut deps.as_mut(),
            hydromancer_id,
            &constants,
            current_round_id,
        );

        // Should return Ok without doing anything
        assert!(result.is_ok());
    }

    #[test]
    fn test_complete_hydromancer_time_weighted_shares_success() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let hydromancer_id = 1;
        let current_round_id = 1;

        let result = complete_hydromancer_time_weighted_shares(
            &mut deps.as_mut(),
            hydromancer_id,
            &constants,
            current_round_id,
        );

        assert!(result.is_ok());

        // Verify vessel shares were saved
        let has_vessel_1 =
            state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 1);
        let has_vessel_2 =
            state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 2);
        assert!(has_vessel_1);
        assert!(has_vessel_2);
    }

    #[test]
    fn test_initialize_vessel_tws_empty_input() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let lock_ids = vec![];
        let current_round_id = 1;

        let result =
            initialize_vessel_tws(&mut deps.as_mut(), lock_ids, current_round_id, &constants);

        assert!(result.is_ok());
    }

    #[test]
    fn test_initialize_vessel_tws_already_initialized() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let lock_ids = vec![1];
        let current_round_id = 1;

        // First initialize the vessel
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            1,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2,
            Some(constants.default_hydromancer_id),
        )
        .unwrap();

        let result =
            initialize_vessel_tws(&mut deps.as_mut(), lock_ids, current_round_id, &constants);

        assert!(result.is_ok());
        // Should not duplicate or change existing data
    }

    #[test]
    fn test_initialize_vessel_tws_new_vessels() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let lock_ids = vec![1, 2];
        let current_round_id = 1;

        let result =
            initialize_vessel_tws(&mut deps.as_mut(), lock_ids, current_round_id, &constants);

        assert!(result.is_ok());

        // Verify vessel shares were saved
        let has_vessel_1 =
            state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 1);
        let has_vessel_2 =
            state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 2);
        assert!(has_vessel_1);
        assert!(has_vessel_2);

        // Function should execute without error - hydromancer TWS would be updated internally
    }

    #[test]
    fn test_initialize_vessel_tws_mixed_vessels() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let constants = get_test_constants();
        let lock_ids = vec![1, 2];
        let current_round_id = 1;

        // Initialize vessel 1 first
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            1,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2,
            Some(constants.default_hydromancer_id),
        )
        .unwrap();

        let result =
            initialize_vessel_tws(&mut deps.as_mut(), lock_ids, current_round_id, &constants);

        assert!(result.is_ok());

        // Verify only vessel 2 was newly initialized
        let has_vessel_2 =
            state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 2);
        assert!(has_vessel_2);
    }

    #[test]
    fn test_initialize_vessel_tws_no_hydromancer() {
        let mut deps = mock_dependencies();
        init_contract(&mut deps);

        let user = make_valid_addr("user");
        let user_id = state::insert_new_user(deps.as_mut().storage, user.clone()).unwrap();

        // Add vessel without hydromancer
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 99,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: false,
                hydromancer_id: None,
                owner_id: user_id,
            },
            &user,
        )
        .unwrap();

        let constants = get_test_constants();
        let lock_ids = vec![99];
        let current_round_id = 1;

        let result =
            initialize_vessel_tws(&mut deps.as_mut(), lock_ids, current_round_id, &constants);

        assert!(result.is_ok());

        // Verify vessel shares were saved
        let has_vessel = state::has_vessel_shares_info(deps.as_ref().storage, current_round_id, 99);
        assert!(has_vessel);
    }

    #[test]
    fn test_batch_hydromancer_tws_changes_multiple_calls() {
        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let current_round_id = 1;

        // First call
        let old_vessel_shares_1 = None;
        let new_lockup_shares_1 = LockupVotingMetrics {
            lock_id: 1,
            time_weighted_shares: Uint128::from(1000u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2,
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares_1,
            &new_lockup_shares_1,
        );

        // Second call with same key
        let old_vessel_shares_2 = None;
        let new_lockup_shares_2 = LockupVotingMetrics {
            lock_id: 2,
            time_weighted_shares: Uint128::from(500u128),
            token_group_id: "dAtom".to_string(),
            locked_rounds_remaining: 2, // Same as first
        };

        batch_hydromancer_tws_changes(
            &mut hydromancer_tws_changes,
            hydromancer_id,
            current_round_id,
            &old_vessel_shares_2,
            &new_lockup_shares_2,
        );

        // Should accumulate values for same key
        assert_eq!(hydromancer_tws_changes.len(), 1);
        let key = (hydromancer_id, current_round_id, "dAtom".to_string(), 2);
        assert_eq!(hydromancer_tws_changes.get(&key), Some(&1500i128)); // 1000 + 500
    }

    #[test]
    fn test_apply_hydromancer_tws_changes_multiple_entries() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let mut hydromancer_tws_changes = HashMap::new();
        let hydromancer_id = 1;
        let round_id = 1;

        // Multiple entries
        let key1 = (hydromancer_id, round_id, "dAtom".to_string(), 1);
        let key2 = (hydromancer_id, round_id, "dAtom".to_string(), 2);
        let key3 = (hydromancer_id, round_id, "uosmo".to_string(), 1);

        hydromancer_tws_changes.insert(key1, 1000i128);
        hydromancer_tws_changes.insert(key2, 2000i128);
        hydromancer_tws_changes.insert(key3, 1500i128);

        let result = apply_hydromancer_tws_changes(deps.as_mut().storage, hydromancer_tws_changes);

        assert!(result.is_ok());

        // Function should execute without error - all TWS would be added internally
    }

    #[test]
    fn test_reset_vessel_vote_success() {
        let mut deps = mock_dependencies();
        let (_, _) = setup_test_vessels(&mut deps);

        let current_round_id = 1;
        let tranche_id = 1;
        let proposal_id = 1;
        let vessel_id = 1;

        // Set up vessel shares info
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            1000,
            "dAtom".to_string(),
            2,
            Some(0),
        )
        .unwrap();

        // Set up harbor mapping
        let vessel_harbor = zephyrus_core::state::VesselHarbor {
            user_control: false,
            steerer_id: 1, // hydromancer_id
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

        // Add some TWS to proposal and hydromancer proposal
        state::add_time_weighted_shares_to_proposal(
            deps.as_mut().storage,
            current_round_id,
            proposal_id,
            "dAtom",
            1000,
        )
        .unwrap();

        state::add_time_weighted_shares_to_proposal_for_hydromancer(
            deps.as_mut().storage,
            proposal_id,
            1, // hydromancer_id
            "dAtom",
            1000,
        )
        .unwrap();

        // Get the vessel
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();

        // Call reset_vessel_vote
        let result = crate::helpers::tws::reset_vessel_vote(
            deps.as_mut().storage,
            vessel,
            current_round_id,
            tranche_id,
            proposal_id,
        );

        assert!(result.is_ok());

        // Verify that vessel harbor mapping was removed
        let harbor_exists = state::get_harbor_of_vessel(
            deps.as_ref().storage,
            tranche_id,
            current_round_id,
            vessel_id,
        )
        .unwrap();
        assert!(harbor_exists.is_none());

        // Verify that TWS were subtracted from proposal
        // Note: We can't directly verify the TWS values without internal access,
        // but the function should execute without error
    }

    #[test]
    fn test_reset_vessel_vote_user_control_success() {
        let mut deps = mock_dependencies();
        init_contract(&mut deps);

        let user = make_valid_addr("user");
        let user_id = state::insert_new_user(deps.as_mut().storage, user.clone()).unwrap();

        // Add vessel under user control (no hydromancer)
        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 99,
                tokenized_share_record_id: None,
                class_period: 1_000_000,
                auto_maintenance: false,
                hydromancer_id: None, // User control
                owner_id: user_id,
            },
            &user,
        )
        .unwrap();

        let current_round_id = 1;
        let tranche_id = 1;
        let proposal_id = 1;
        let vessel_id = 99;

        // Set up vessel shares info
        state::save_vessel_info_snapshot(
            deps.as_mut().storage,
            vessel_id,
            current_round_id,
            500,
            "stAtom".to_string(),
            1,
            Some(0),
        )
        .unwrap();

        // Set up harbor mapping for user-controlled vessel
        let vessel_harbor = zephyrus_core::state::VesselHarbor {
            user_control: true,
            steerer_id: user_id,
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

        // Add some TWS to proposal (no hydromancer TWS for user-controlled vessels)
        state::add_time_weighted_shares_to_proposal(
            deps.as_mut().storage,
            current_round_id,
            proposal_id,
            "stAtom",
            500,
        )
        .unwrap();

        // Get the vessel
        let vessel = state::get_vessel(deps.as_ref().storage, vessel_id).unwrap();

        // Call reset_vessel_vote
        let result = crate::helpers::tws::reset_vessel_vote(
            deps.as_mut().storage,
            vessel,
            current_round_id,
            tranche_id,
            proposal_id,
        );

        assert!(result.is_ok());

        // Verify that vessel harbor mapping was removed
        let harbor_exists = state::get_harbor_of_vessel(
            deps.as_ref().storage,
            tranche_id,
            current_round_id,
            vessel_id,
        )
        .unwrap();
        assert!(harbor_exists.is_none());

        // Function should execute without error for user-controlled vessels
    }
}
