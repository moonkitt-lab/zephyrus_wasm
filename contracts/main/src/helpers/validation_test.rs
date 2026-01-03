#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::mock_env, Addr, Decimal, MessageInfo, Timestamp};
    use hydro_interface::msgs::{
        LockEntryV2, LockEntryWithPower, LockPowerEntry, LockupWithPerTrancheInfo,
        PerTrancheLockupInfo, RoundLockPowerSchedule,
    };
    use zephyrus_core::msgs::{InstantiateMsg, VesselsToHarbor};
    use zephyrus_core::state::{Constants, HydroConfig, Vessel};

    use crate::helpers::validation::validate_user_controls_vessel;
    use crate::{
        errors::ContractError,
        helpers::validation::{
            validate_admin_address, validate_commission_rate, validate_contract_is_not_paused,
            validate_contract_is_paused, validate_hydromancer_controls_vessels,
            validate_hydromancer_exists, validate_lock_duration, validate_no_duplicate_ids,
            validate_user_owns_vessels, validate_vessels_not_tied_to_proposal,
            validate_vessels_under_user_control, validate_vote_duplicates,
        },
        state,
        testing::make_valid_addr,
        testing_mocks::mock_dependencies,
    };

    fn get_test_constants(paused: bool) -> Constants {
        Constants {
            default_hydromancer_id: 0,
            paused_contract: paused,
            hydro_config: HydroConfig {
                hydro_contract_address: make_valid_addr("hydro"),
                hydro_tribute_contract_address: make_valid_addr("tribute"),
                hydro_governance_proposal_address: make_valid_addr("hydro_gov"),
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
                hydro_governance_proposal_address: make_valid_addr("hydro_gov").into_string(),
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
    ) -> (Addr, Addr, u64, Addr) {
        init_contract(deps);

        let user1 = make_valid_addr("user1");
        let user2 = make_valid_addr("user2");
        let hydromancer_addr = make_valid_addr("hydromancer");

        let user1_id = state::insert_new_user(deps.as_mut().storage, user1.clone()).unwrap();
        let user2_id = state::insert_new_user(deps.as_mut().storage, user2.clone()).unwrap();

        // Create hydromancer
        let hydromancer_id = state::insert_new_hydromancer(
            deps.as_mut().storage,
            hydromancer_addr.clone(),
            "Test Hydromancer".to_string(),
            "0.1".parse().unwrap(),
        )
        .unwrap();

        // Add vessels - some under hydromancer control, some under user control
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
                auto_maintenance: false,
                hydromancer_id: None, // Under user control
                owner_id: user2_id,
            },
            &user2,
        )
        .unwrap();

        state::add_vessel(
            deps.as_mut().storage,
            &Vessel {
                hydro_lock_id: 3,
                tokenized_share_record_id: None,
                class_period: 3_000_000,
                auto_maintenance: false,
                hydromancer_id: None, // Under user control
                owner_id: user1_id,
            },
            &user1,
        )
        .unwrap();

        (user1, user2, hydromancer_id, hydromancer_addr)
    }

    #[test]
    fn test_validate_contract_is_not_paused_success() {
        let constants = get_test_constants(false);
        let result = validate_contract_is_not_paused(&constants);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_contract_is_not_paused_failure() {
        let constants = get_test_constants(true);
        let result = validate_contract_is_not_paused(&constants);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ContractError::Paused));
    }

    #[test]
    fn test_validate_contract_is_paused_success() {
        let constants = get_test_constants(true);
        let result = validate_contract_is_paused(&constants);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_contract_is_paused_failure() {
        let constants = get_test_constants(false);
        let result = validate_contract_is_paused(&constants);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ContractError::NotPaused));
    }

    #[test]
    fn test_validate_hydromancer_exists_success() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer_id, _) = setup_test_data(&mut deps);

        let result = validate_hydromancer_exists(deps.as_ref().storage, hydromancer_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hydromancer_exists_failure() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let non_existent_id = 999;
        let result = validate_hydromancer_exists(deps.as_ref().storage, non_existent_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::HydromancerNotFound { .. }
        ));
    }

    #[test]
    fn test_validate_vessels_under_user_control_success() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let user_controlled_vessels = vec![2, 3]; // Vessels without hydromancer_id
        let result =
            validate_vessels_under_user_control(deps.as_ref().storage, &user_controlled_vessels);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vessels_under_user_control_failure() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let mixed_vessels = vec![1, 2]; // Vessel 1 is under hydromancer control
        let result = validate_vessels_under_user_control(deps.as_ref().storage, &mixed_vessels);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::VesselUnderHydromancerControl { vessel_id: 1 }
        ));
    }

    #[test]
    fn test_validate_vessels_under_user_control_empty_list() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let empty_vessels = vec![];
        let result = validate_vessels_under_user_control(deps.as_ref().storage, &empty_vessels);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vote_duplicates_success() {
        let vessels_harbors = vec![
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![1, 2],
            },
            VesselsToHarbor {
                harbor_id: 2,
                vessel_ids: vec![3, 4],
            },
        ];

        let result = validate_vote_duplicates(&vessels_harbors);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vote_duplicates_duplicate_harbor() {
        let vessels_harbors = vec![
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![1, 2],
            },
            VesselsToHarbor {
                harbor_id: 1, // Duplicate harbor ID
                vessel_ids: vec![3, 4],
            },
        ];

        let result = validate_vote_duplicates(&vessels_harbors);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::DuplicateHarborId { harbor_id: 1 }
        ));
    }

    #[test]
    fn test_validate_vote_duplicates_duplicate_vessel() {
        let vessels_harbors = vec![
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![1, 2],
            },
            VesselsToHarbor {
                harbor_id: 2,
                vessel_ids: vec![2, 3], // Vessel 2 is duplicate
            },
        ];

        let result = validate_vote_duplicates(&vessels_harbors);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::DuplicateVesselId { vessel_id: 2 }
        ));
    }

    #[test]
    fn test_validate_vote_duplicates_vessel_within_same_harbor() {
        let vessels_harbors = vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![1, 2, 2], // Duplicate vessel within same harbor
        }];

        let result = validate_vote_duplicates(&vessels_harbors);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::DuplicateVesselId { vessel_id: 2 }
        ));
    }

    #[test]
    fn test_validate_vote_duplicates_empty_list() {
        let vessels_harbors = vec![];
        let result = validate_vote_duplicates(&vessels_harbors);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_duplicate_ids_success() {
        let ids = vec![1, 2, 3, 4, 5];
        let result = validate_no_duplicate_ids(&ids, "Vessel");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_duplicate_ids_vessel_duplicate() {
        let ids = vec![1, 2, 3, 2, 5];
        let result = validate_no_duplicate_ids(&ids, "Vessel");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::DuplicateVesselId { vessel_id: 2 }
        ));
    }

    #[test]
    fn test_validate_no_duplicate_ids_harbor_duplicate() {
        let ids = vec![1, 2, 3, 1, 5];
        let result = validate_no_duplicate_ids(&ids, "Harbor");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::DuplicateHarborId { harbor_id: 1 }
        ));
    }

    #[test]
    fn test_validate_no_duplicate_ids_custom_type() {
        let ids = vec![1, 2, 3, 2, 5];
        let result = validate_no_duplicate_ids(&ids, "Custom");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::CustomError { .. }
        ));
    }

    #[test]
    fn test_validate_no_duplicate_ids_empty_list() {
        let ids = vec![];
        let result = validate_no_duplicate_ids(&ids, "Vessel");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_duplicate_ids_single_element() {
        let ids = vec![1];
        let result = validate_no_duplicate_ids(&ids, "Vessel");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_admin_address_success() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let admin = make_valid_addr("admin");
        let result = validate_admin_address(deps.as_ref().storage, &admin);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_admin_address_failure() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        let non_admin = make_valid_addr("user");
        let result = validate_admin_address(deps.as_ref().storage, &non_admin);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::Unauthorized {}
        ));
    }

    #[test]
    fn test_validate_user_owns_vessels_success() {
        let mut deps = mock_dependencies();
        let (user1, user2, _, _) = setup_test_data(&mut deps);

        // User1 owns vessels 1 and 3
        let result = validate_user_owns_vessels(deps.as_ref().storage, &user1, &[1, 3]);
        assert!(result.is_ok());

        // User2 owns vessel 2
        let result = validate_user_owns_vessels(deps.as_ref().storage, &user2, &[2]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_user_owns_vessels_failure() {
        let mut deps = mock_dependencies();
        let (user1, user2, _, _) = setup_test_data(&mut deps);

        // User1 doesn't own vessel 2
        let result = validate_user_owns_vessels(deps.as_ref().storage, &user1, &[2]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::Unauthorized {}
        ));

        // User2 doesn't own vessel 1
        let result = validate_user_owns_vessels(deps.as_ref().storage, &user2, &[1, 3]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::Unauthorized {}
        ));
    }

    #[test]
    fn test_validate_user_owns_vessels_empty_list() {
        let mut deps = mock_dependencies();
        let (user1, _, _, _) = setup_test_data(&mut deps);

        let result = validate_user_owns_vessels(deps.as_ref().storage, &user1, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hydromancer_controls_vessels_success() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer_id, _) = setup_test_data(&mut deps);

        // Hydromancer controls vessel 1
        let result =
            validate_hydromancer_controls_vessels(deps.as_ref().storage, hydromancer_id, &[1]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hydromancer_controls_vessels_failure() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer_id, _) = setup_test_data(&mut deps);

        // Hydromancer doesn't control vessel 2 (under user control)
        let result =
            validate_hydromancer_controls_vessels(deps.as_ref().storage, hydromancer_id, &[2]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::Unauthorized {}
        ));

        // Mixed vessels - some controlled, some not
        let result =
            validate_hydromancer_controls_vessels(deps.as_ref().storage, hydromancer_id, &[1, 2]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::Unauthorized {}
        ));
    }

    #[test]
    fn test_validate_hydromancer_controls_vessels_empty_list() {
        let mut deps = mock_dependencies();
        let (_, _, hydromancer_id, _) = setup_test_data(&mut deps);

        let result =
            validate_hydromancer_controls_vessels(deps.as_ref().storage, hydromancer_id, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vessels_not_tied_to_proposal_success() {
        let lockups_with_per_tranche_infos = vec![
            LockupWithPerTrancheInfo {
                lock_with_power: LockEntryWithPower {
                    lock_entry: LockEntryV2 {
                        lock_id: 1,
                        owner: make_valid_addr("owner1"),
                        funds: cosmwasm_std::coin(1000, "uatom"),
                        lock_start: Timestamp::from_seconds(1000),
                        lock_end: Timestamp::from_seconds(2000),
                    },
                    current_voting_power: cosmwasm_std::Uint128::from(1000u128),
                },
                per_tranche_info: vec![PerTrancheLockupInfo {
                    tranche_id: 1,
                    next_round_lockup_can_vote: 2,
                    current_voted_on_proposal: None,
                    tied_to_proposal: None, // Not tied to proposal
                    historic_voted_on_proposals: vec![],
                }],
            },
            LockupWithPerTrancheInfo {
                lock_with_power: LockEntryWithPower {
                    lock_entry: LockEntryV2 {
                        lock_id: 2,
                        owner: make_valid_addr("owner2"),
                        funds: cosmwasm_std::coin(2000, "uatom"),
                        lock_start: Timestamp::from_seconds(1000),
                        lock_end: Timestamp::from_seconds(2000),
                    },
                    current_voting_power: cosmwasm_std::Uint128::from(2000u128),
                },
                per_tranche_info: vec![PerTrancheLockupInfo {
                    tranche_id: 1,
                    next_round_lockup_can_vote: 2,
                    current_voted_on_proposal: None,
                    tied_to_proposal: None, // Not tied to proposal
                    historic_voted_on_proposals: vec![],
                }],
            },
        ];

        let result = validate_vessels_not_tied_to_proposal(&lockups_with_per_tranche_infos);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vessels_not_tied_to_proposal_failure() {
        let lockups_with_per_tranche_infos = vec![LockupWithPerTrancheInfo {
            lock_with_power: LockEntryWithPower {
                lock_entry: LockEntryV2 {
                    lock_id: 1,
                    owner: make_valid_addr("owner1"),
                    funds: cosmwasm_std::coin(1000, "uatom"),
                    lock_start: Timestamp::from_seconds(1000),
                    lock_end: Timestamp::from_seconds(2000),
                },
                current_voting_power: cosmwasm_std::Uint128::from(1000u128),
            },
            per_tranche_info: vec![PerTrancheLockupInfo {
                tranche_id: 1,
                next_round_lockup_can_vote: 2,
                current_voted_on_proposal: None,
                tied_to_proposal: Some(123), // Tied to proposal
                historic_voted_on_proposals: vec![],
            }],
        }];

        let result = validate_vessels_not_tied_to_proposal(&lockups_with_per_tranche_infos);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::VesselTiedToProposalNotTransferable { vessel_id: 1 }
        ));
    }

    #[test]
    fn test_validate_vessels_not_tied_to_proposal_empty_list() {
        let lockups_with_per_tranche_infos = vec![];
        let result = validate_vessels_not_tied_to_proposal(&lockups_with_per_tranche_infos);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vessels_not_tied_to_proposal_multiple_tranches() {
        let lockups_with_per_tranche_infos = vec![LockupWithPerTrancheInfo {
            lock_with_power: LockEntryWithPower {
                lock_entry: LockEntryV2 {
                    lock_id: 1,
                    owner: make_valid_addr("owner1"),
                    funds: cosmwasm_std::coin(1000, "uatom"),
                    lock_start: Timestamp::from_seconds(1000),
                    lock_end: Timestamp::from_seconds(2000),
                },
                current_voting_power: cosmwasm_std::Uint128::from(1000u128),
            },
            per_tranche_info: vec![
                PerTrancheLockupInfo {
                    tranche_id: 1,
                    next_round_lockup_can_vote: 2,
                    current_voted_on_proposal: None,
                    tied_to_proposal: None, // Not tied
                    historic_voted_on_proposals: vec![],
                },
                PerTrancheLockupInfo {
                    tranche_id: 2,
                    next_round_lockup_can_vote: 2,
                    current_voted_on_proposal: None,
                    tied_to_proposal: Some(456), // Tied to proposal
                    historic_voted_on_proposals: vec![],
                },
            ],
        }];

        let result = validate_vessels_not_tied_to_proposal(&lockups_with_per_tranche_infos);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::VesselTiedToProposalNotTransferable { vessel_id: 1 }
        ));
    }

    #[test]
    fn test_validate_lock_duration_success() {
        let round_lock_power_schedule = RoundLockPowerSchedule {
            round_lock_power_schedule: vec![
                LockPowerEntry {
                    locked_rounds: 1,
                    power_scaling_factor: cosmwasm_std::Decimal::one(),
                },
                LockPowerEntry {
                    locked_rounds: 2,
                    power_scaling_factor: cosmwasm_std::Decimal::from_ratio(5u128, 4u128),
                },
                LockPowerEntry {
                    locked_rounds: 3,
                    power_scaling_factor: cosmwasm_std::Decimal::from_ratio(3u128, 2u128),
                },
            ],
        };

        let lock_epoch_length = 1_000_000;

        // Valid durations: 1 * 1_000_000 = 1_000_000, 2 * 1_000_000 = 2_000_000, 3 * 1_000_000 = 3_000_000
        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_000_000);
        assert!(result.is_ok());

        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 2_000_000);
        assert!(result.is_ok());

        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 3_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_lock_duration_failure() {
        let round_lock_power_schedule = RoundLockPowerSchedule {
            round_lock_power_schedule: vec![
                LockPowerEntry {
                    locked_rounds: 1,
                    power_scaling_factor: cosmwasm_std::Decimal::one(),
                },
                LockPowerEntry {
                    locked_rounds: 2,
                    power_scaling_factor: cosmwasm_std::Decimal::from_ratio(5u128, 4u128),
                },
                LockPowerEntry {
                    locked_rounds: 3,
                    power_scaling_factor: cosmwasm_std::Decimal::from_ratio(3u128, 2u128),
                },
            ],
        };

        let lock_epoch_length = 1_000_000;

        // Invalid duration: 1_500_000 is not in the valid list
        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_500_000);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::InvalidLockDuration { .. }
        ));
    }

    #[test]
    fn test_validate_lock_duration_empty_schedule() {
        let round_lock_power_schedule = RoundLockPowerSchedule {
            round_lock_power_schedule: vec![],
        };

        let lock_epoch_length = 1_000_000;

        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_000_000);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::InvalidLockDuration { .. }
        ));
    }

    #[test]
    fn test_validate_lock_duration_different_epoch_length() {
        let round_lock_power_schedule = RoundLockPowerSchedule {
            round_lock_power_schedule: vec![
                LockPowerEntry {
                    locked_rounds: 1,
                    power_scaling_factor: cosmwasm_std::Decimal::one(),
                },
                LockPowerEntry {
                    locked_rounds: 2,
                    power_scaling_factor: cosmwasm_std::Decimal::from_ratio(5u128, 4u128),
                },
            ],
        };

        let lock_epoch_length = 500_000;

        // Valid durations: 1 * 500_000 = 500_000, 2 * 500_000 = 1_000_000
        let result = validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 500_000);
        assert!(result.is_ok());

        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_000_000);
        assert!(result.is_ok());

        // Invalid duration with different epoch length
        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_500_000);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::InvalidLockDuration { .. }
        ));
    }

    #[test]
    fn test_validate_lock_duration_zero_epoch_length() {
        let round_lock_power_schedule = RoundLockPowerSchedule {
            round_lock_power_schedule: vec![LockPowerEntry {
                locked_rounds: 1,
                power_scaling_factor: cosmwasm_std::Decimal::one(),
            }],
        };

        let lock_epoch_length = 0; // Actually, this should not be possible to have lock_epoch_length = 0

        // With zero epoch length, valid duration is 1 * 0 = 0
        let result = validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 0);
        assert!(result.is_ok());

        let result =
            validate_lock_duration(&round_lock_power_schedule, lock_epoch_length, 1_000_000);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContractError::InvalidLockDuration { .. }
        ));
    }

    #[test]
    fn test_validation_integration_multiple_checks() {
        let mut deps = mock_dependencies();
        let (user1, user2, hydromancer_id, _) = setup_test_data(&mut deps);

        // Test multiple validation functions together
        let constants = get_test_constants(false);

        // Contract should not be paused
        assert!(validate_contract_is_not_paused(&constants).is_ok());

        // Hydromancer should exist
        assert!(validate_hydromancer_exists(deps.as_ref().storage, hydromancer_id).is_ok());

        // User should own their vessels
        assert!(validate_user_owns_vessels(deps.as_ref().storage, &user1, &[1, 3]).is_ok());
        assert!(validate_user_owns_vessels(deps.as_ref().storage, &user2, &[2]).is_ok());

        // Hydromancer should control their vessels
        assert!(
            validate_hydromancer_controls_vessels(deps.as_ref().storage, hydromancer_id, &[1])
                .is_ok()
        );

        // Vessels under user control should validate correctly
        assert!(validate_vessels_under_user_control(deps.as_ref().storage, &[2, 3]).is_ok());

        // Admin should be validated
        let admin = make_valid_addr("admin");
        assert!(validate_admin_address(deps.as_ref().storage, &admin).is_ok());
    }

    #[test]
    fn test_validation_edge_cases_large_numbers() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        // Test with large vessel IDs
        let large_ids = vec![u64::MAX - 1, u64::MAX];
        let result = validate_no_duplicate_ids(&large_ids, "Vessel");
        assert!(result.is_ok());

        // Test with duplicate large IDs
        let duplicate_large_ids = vec![u64::MAX, u64::MAX - 1, u64::MAX];
        let result = validate_no_duplicate_ids(&duplicate_large_ids, "Vessel");
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_edge_cases_zero_values() {
        let mut deps = mock_dependencies();
        let (_, _, _, _) = setup_test_data(&mut deps);

        // Test with zero IDs
        let zero_ids = vec![0, 1, 2];
        let result = validate_no_duplicate_ids(&zero_ids, "Vessel");
        assert!(result.is_ok());

        // Test with duplicate zero
        let duplicate_zero_ids = vec![0, 1, 0];
        let result = validate_no_duplicate_ids(&duplicate_zero_ids, "Vessel");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_user_controls_vessel_success() {
        let mut deps = mock_dependencies();
        let (user1, _, _, _) = setup_test_data(&mut deps);

        let user1_id = state::get_user_id(deps.as_ref().storage, &user1).unwrap();
        // Test with user controlling vessel
        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: None,
            owner_id: user1_id,
        };
        let result = validate_user_controls_vessel(deps.as_ref().storage, user1, vessel);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_hydromancer_controls_vessel_success() {
        let mut deps = mock_dependencies();
        let (user1, _, hydromancer_id, hydromancer_addr) = setup_test_data(&mut deps);

        let user1_id = state::get_user_id(deps.as_ref().storage, &user1).unwrap();
        // Test with user controlling vessel
        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };
        let result = validate_user_controls_vessel(deps.as_ref().storage, hydromancer_addr, vessel);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_user_controls_vessel_fail() {
        let mut deps = mock_dependencies();
        let (user1, user2, hydromancer_id, _) = setup_test_data(&mut deps);

        let user1_id = state::get_user_id(deps.as_ref().storage, &user1).unwrap();
        // Test with user controlling vessel
        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };
        let result = validate_user_controls_vessel(deps.as_ref().storage, user2.clone(), vessel);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_hydromancer_controls_vessel_fail() {
        let mut deps = mock_dependencies();
        let (user1, _, hydromancer_id, _) = setup_test_data(&mut deps);
        let bad_hydromancer_addr = make_valid_addr("new_hydromancer");
        state::insert_new_hydromancer(
            deps.as_mut().storage,
            bad_hydromancer_addr.clone(),
            "New Hydromancer".to_string(),
            "0.1".parse().unwrap(),
        )
        .unwrap();
        let user1_id = state::get_user_id(deps.as_ref().storage, &user1).unwrap();
        // Test with user controlling vessel
        let vessel = Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: None,
            class_period: 1_000_000,
            auto_maintenance: true,
            hydromancer_id: Some(hydromancer_id),
            owner_id: user1_id,
        };
        let result = validate_user_controls_vessel(
            deps.as_ref().storage,
            bad_hydromancer_addr.clone(),
            vessel,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_commission_rate_success() {
        // Test with valid commission rates (0 to 0.5)
        let valid_rates = vec![
            Decimal::zero(),
            Decimal::from_ratio(1u128, 100u128),  // 1%
            Decimal::from_ratio(10u128, 100u128), // 10%
            Decimal::from_ratio(25u128, 100u128), // 25%
            Decimal::from_ratio(49u128, 100u128), // 49%
        ];

        for rate in valid_rates {
            let result = validate_commission_rate(rate);
            assert!(result.is_ok(), "Commission rate {:?} should be valid", rate);
        }
    }

    #[test]
    fn test_validate_commission_rate_too_high() {
        // Test with commission rate >= 0.5 (50%) (should fail)
        let too_high_rates = vec![
            Decimal::from_ratio(50u128, 100u128),  // 50% - should fail
            Decimal::from_ratio(51u128, 100u128),  // 51% - should fail
            Decimal::from_ratio(75u128, 100u128),  // 75% - should fail
            Decimal::one(),                        // 100% - should fail
            Decimal::from_ratio(150u128, 100u128), // 150% - should fail
        ];

        for rate in too_high_rates {
            let result = validate_commission_rate(rate);
            assert!(result.is_err(), "Commission rate {:?} should fail", rate);

            match result.unwrap_err() {
                ContractError::CommissionRateMustBeLessThanMax {
                    max_commission_rate,
                } => {
                    assert_eq!(max_commission_rate, Decimal::from_ratio(50u128, 100u128));
                }
                _ => panic!("Expected CommissionRateMustBeLessThanMax error"),
            }
        }
    }

    #[test]
    fn test_validate_commission_rate_edge_cases() {
        // Test with exactly 0.5 (50%) - should fail (>= max)
        let exactly_max = Decimal::from_ratio(50u128, 100u128);
        let result = validate_commission_rate(exactly_max);
        assert!(result.is_err(), "Commission rate exactly 0.5 should fail");

        // Test with value just below 0.5 (should succeed)
        let just_below_max = Decimal::from_ratio(499999u128, 1000000u128); // 0.499999
        let result = validate_commission_rate(just_below_max);
        assert!(
            result.is_ok(),
            "Commission rate just below 0.5 should succeed"
        );

        // Test with zero (should succeed)
        let zero = Decimal::zero();
        let result = validate_commission_rate(zero);
        assert!(result.is_ok(), "Zero commission rate should succeed");
    }

    #[test]
    fn test_validate_commission_rate_boundary_values() {
        // Test boundary values around 0.5
        let max_rate = Decimal::from_ratio(50u128, 100u128);

        // Just below max (should succeed)
        let just_below = max_rate - Decimal::from_ratio(1u128, 1000000u128);
        let result = validate_commission_rate(just_below);
        assert!(result.is_ok(), "Value just below max should succeed");

        // Exactly at max (should fail)
        let result = validate_commission_rate(max_rate);
        assert!(result.is_err(), "Value exactly at max should fail");

        // Just above max (should fail)
        let just_above = max_rate + Decimal::from_ratio(1u128, 1000000u128);
        let result = validate_commission_rate(just_above);
        assert!(result.is_err(), "Value just above max should fail");
    }
}
