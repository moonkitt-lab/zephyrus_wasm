use crate::helpers::hydro_queries::query_hydro_lockups_shares;
use crate::reply::handle_refresh_time_weighted_shares_reply;
use crate::testing_mocks::{mock_dependencies, mock_hydro_contract};
use crate::{
    contract::{execute, instantiate},
    errors::ContractError,
    reply::{handle_claim_tribute_reply, handle_vote_reply},
    state::{self},
};
use cosmwasm_std::{from_json, CosmosMsg, DepsMut, ReplyOn, WasmMsg};
use cosmwasm_std::{
    testing::{message_info, mock_env, MockApi},
    to_json_binary, Addr, Binary, Coin, Decimal, MessageInfo,
};
use hydro_interface::msgs::ExecuteMsg as HydroExecuteMsg;
use zephyrus_core::msgs::{
    ClaimTributeReplyPayload, Cw721ReceiveMsg, ExecuteMsg, InstantiateMsg,
    RefreshTimeWeightedSharesReplyPayload, VesselInfo, VesselsToHarbor, VoteReplyPayload,
};
use zephyrus_core::state::{Vessel, VesselHarbor};

pub fn get_address_as_str(mock_api: &MockApi, addr: &str) -> String {
    mock_api.addr_make(addr).to_string()
}

pub fn make_valid_addr(addr: &str) -> Addr {
    MockApi::default().addr_make(addr)
}

#[test]
fn instantiate_test() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let user_address = get_address_as_str(&deps.api, "addr0000");
    let msg = get_default_instantiate_msg(&deps, user_address);
    let res = instantiate(deps.as_mut(), env, info, msg);
    assert!(res.is_ok(), "error: {:?}", res);
}

fn get_default_instantiate_msg(
    deps: &cosmwasm_std::OwnedDeps<
        cosmwasm_std::MemoryStorage,
        MockApi,
        crate::testing_mocks::MockQuerier,
    >,
    user_address: String,
) -> InstantiateMsg {
    let msg = InstantiateMsg {
        whitelist_admins: vec![user_address.clone()],

        hydro_contract_address: get_address_as_str(&deps.api, "hydro_addr"),
        tribute_contract_address: get_address_as_str(&deps.api, "tribute_addr"),
        default_hydromancer_address: get_address_as_str(&deps.api, "hydromancer_addr"),
        default_hydromancer_name: get_address_as_str(&deps.api, "default_hydromancer_name"),
        default_hydromancer_commission_rate: Decimal::from_ratio(1u128, 100u128),
        commission_rate: "0.1".parse().unwrap(),
        commission_recipient: get_address_as_str(&deps.api, "commission_recipient"),
    };
    msg
}

#[test]
fn pause_fail_not_admin() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok(), "error: {:?}", res);
    let info1 = message_info(&Addr::unchecked("sender"), &[]);

    let msg = ExecuteMsg::PauseContract {};

    let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Unauthorized.to_string()
    );
}

#[test]
fn unpause_fail_not_admin() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok(), "error: {:?}", res);
    let info1 = message_info(&Addr::unchecked("sender"), &[]);

    let msg = ExecuteMsg::UnpauseContract {};

    let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Unauthorized.to_string()
    );
}

#[test]
fn pause_basic_test() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok(), "error: {:?}", res);
    let info1 = message_info(&Addr::unchecked(admin_address.clone()), &[]);

    let msg_pause = ExecuteMsg::PauseContract {};

    let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg_pause);
    assert!(res.is_ok(), "error: {:?}", res);

    //now every msg executed should be in error "ContractError::Paused"
    let info2 = message_info(&Addr::unchecked("sender"), &[]);
    let msg_receive_nft = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
        sender: Addr::unchecked("sender").to_string(),
        token_id: "1".to_string(),
        msg: Binary::from("{}".as_bytes()),
    });
    let res = execute(deps.as_mut(), env.clone(), info2.clone(), msg_receive_nft);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Paused.to_string()
    );
    let info3 = message_info(&Addr::unchecked("sender"), &[]);
    let msg_auto_maintain = ExecuteMsg::AutoMaintain {
        start_from_vessel_id: None,
        limit: None,
    };
    let res = execute(deps.as_mut(), env.clone(), info3.clone(), msg_auto_maintain);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Paused.to_string()
    );

    let info4 = message_info(&Addr::unchecked("sender"), &[]);
    let msg_modify_automaintenance = ExecuteMsg::ModifyAutoMaintenance {
        hydro_lock_ids: vec![0],
        auto_maintenance: true,
    };
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info4.clone(),
        msg_modify_automaintenance,
    );
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Paused.to_string()
    );

    let info5 = message_info(&Addr::unchecked("sender"), &[]);
    let msg_update_class = ExecuteMsg::UpdateVesselsClass {
        hydro_lock_ids: vec![1],
        hydro_lock_duration: 1000,
    };
    let res = execute(deps.as_mut(), env.clone(), info5.clone(), msg_update_class);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::Paused.to_string()
    );
}

#[test]
fn fail_unpause_already_unpause_contract_test() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok(), "error: {:?}", res);
    let info1 = message_info(&Addr::unchecked(admin_address.clone()), &[]);

    let msg = ExecuteMsg::UnpauseContract {};

    let res = execute(deps.as_mut(), env.clone(), info1.clone(), msg);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ContractError::NotPaused);
}

#[test]
fn test_cw721_receive_nft_fail_collection_not_accepted() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
    let fake_nft_contract_address = deps.api.addr_make("fake_nft_contract_address");
    let sender = deps.api.addr_make("sender");

    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    let info = MessageInfo {
        sender: fake_nft_contract_address.clone(),
        funds: vec![],
    };
    let receive_msg = Cw721ReceiveMsg {
        sender: sender.to_string(),
        token_id: "1".to_string(),
        msg: Binary::from("{}".as_bytes()),
    };
    let msg = ExecuteMsg::ReceiveNft(receive_msg);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        ContractError::NftNotAccepted.to_string()
    );
}

#[test]
fn test_cw721_receive_nft_fail_bad_period() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
    let hydro_contract = deps.api.addr_make("hydro_addr");
    let sender = deps.api.addr_make("sender");

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    mock_hydro_contract(&mut deps, false);

    let info = MessageInfo {
        sender: hydro_contract.clone(),
        funds: vec![],
    };
    let vessel_info = VesselInfo {
        owner: sender.to_string(),
        auto_maintenance: true,
        hydromancer_id: 0,
        class_period: 6_000_000, // 6 lock_epoch_length
    };
    let receive_msg = Cw721ReceiveMsg {
        sender: sender.to_string(),
        token_id: "1".to_string(),
        msg: to_json_binary(&vessel_info).unwrap(),
    };
    let msg = ExecuteMsg::ReceiveNft(receive_msg);
    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Lock duration must be one of: [1000000, 2000000, 3000000]; but was: 6000000"));
}

#[test]
fn test_cw721_receive_nft_fail_not_owner() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
    let hydro_contract = deps.api.addr_make("hydro_addr");
    let sender = deps.api.addr_make("sender");

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    mock_hydro_contract(&mut deps, true);

    let info = MessageInfo {
        sender: hydro_contract.clone(),
        funds: vec![],
    };
    let vessel_info = VesselInfo {
        owner: sender.to_string(),
        auto_maintenance: true,
        hydromancer_id: 0,
        class_period: 3_000_000, // 3 lock_epoch_length
    };

    let receive_msg = Cw721ReceiveMsg {
        sender: sender.to_string(),
        token_id: "2".to_string(),
        msg: to_json_binary(&vessel_info).unwrap(),
    };
    let msg = ExecuteMsg::ReceiveNft(receive_msg);

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Lockup 2 not owned by Zephyrus"));
}

#[test]
fn test_cw721_receive_nft_succeed() {
    let (mut deps, env) = (mock_dependencies(), mock_env());
    let admin_address = get_address_as_str(&deps.api, "addr0000");
    let info = message_info(&Addr::unchecked("sender"), &[]);
    let msg = get_default_instantiate_msg(&deps, admin_address.to_string());
    let hydro_contract = deps.api.addr_make("hydro_addr");
    let sender = deps.api.addr_make("sender");

    let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());
    assert!(res.is_ok());

    mock_hydro_contract(&mut deps, false);

    let info = MessageInfo {
        sender: hydro_contract.clone(),
        funds: vec![],
    };
    let vessel_info = VesselInfo {
        owner: sender.to_string(),
        auto_maintenance: true,
        hydromancer_id: 0,
        class_period: 3_000_000, // 3 lock_epoch_length
    };
    let receive_msg = Cw721ReceiveMsg {
        sender: sender.to_string(),
        token_id: "1".to_string(),
        msg: to_json_binary(&vessel_info).unwrap(),
    };
    let msg = ExecuteMsg::ReceiveNft(receive_msg);

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);
    assert!(res.is_ok());
}

fn init_contract(deps: DepsMut) {
    instantiate(
        deps,
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
    )
    .unwrap();
}

#[test]
fn hydromancer_vote_fails_not_hydromancer() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    init_contract(deps.as_mut());
    let alice_address = make_valid_addr("alice");

    let info = MessageInfo {
        sender: alice_address.clone(),
        funds: vec![],
    };

    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![1, 2],
            },
            VesselsToHarbor {
                harbor_id: 2,
                vessel_ids: vec![3, 4],
            },
        ],
    };

    let res = execute(deps.as_mut(), env.clone(), info.clone(), msg);

    assert_eq!(
        res.unwrap_err(),
        ContractError::HydromancerNotFound {
            identifier: alice_address.to_string()
        }
    );
}

#[test]
fn hydromancer_vote_with_vessel_controlled_other_hydromancer_fail() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");

    let hydromancer_address = make_valid_addr("hydromancer");

    state::insert_new_hydromancer(
        deps.as_mut().storage,
        hydromancer_address.clone(),
        "hydromancer 1".to_string(),
        Decimal::percent(10),
    )
    .expect("Should add hydromancer");

    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: None,
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(0), // Default hydromancer (not the one created above)
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    // Hydromancer 1 tries to vote with a vessel that is controlled by Zephyrus (hydromancer 0)
    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: hydromancer_address.clone(),
            funds: vec![],
        },
        msg,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn hydromancer_vote_with_vessel_under_user_control_fail() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    let default_hydromancer_address =
        state::get_hydromancer(deps.as_mut().storage, default_hydromancer_id)
            .unwrap()
            .address;

    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: None,
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: None, // under user control
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    // Hydromancer 1 tries to vote with a vessel that is controlled by Zephyrus (hydromancer 0)
    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: default_hydromancer_address,
            funds: vec![],
        },
        msg,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn hydromancer_vote_succeed_without_change_because_vote_skipped_by_hydro() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    state::add_vessel_to_harbor(
        deps.as_mut().storage,
        1,
        1,
        2,
        &VesselHarbor {
            user_control: false,
            hydro_lock_id: 0,
            steerer_id: default_hydromancer_id,
        },
    )
    .expect("Should add vessel to harbor");

    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: make_valid_addr("zephyrus"),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            assert_eq!(
                submsg.reply_on,
                ReplyOn::Success,
                "all lock messages should be reply_on_success"
            );

            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Vote {
        tranche_id,
        proposals_votes,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(proposals_votes.len(), 1);
        assert_eq!(proposals_votes[0].proposal_id, 1);
        assert_eq!(proposals_votes[0].lock_ids, vec![0]);
    } else {
        panic!("Le message ne correspond pas au pattern attendu !");
    }

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: false,
        steerer_id: default_hydromancer_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };
    let skipped_ids = vec![0];
    let _ = handle_vote_reply(deps.as_mut(), payload, skipped_ids).unwrap();

    let vessels_to_harbor2 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor2.len(), 1);
    assert_eq!(vessels_to_harbor2[0].1.hydro_lock_id, 0);
    assert_eq!(vessels_to_harbor2[0].1.steerer_id, default_hydromancer_id);
    //vote should be skipped so harbor1 should not have vessels
    let vessels_to_harbor1 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor1.len(), 0);
}

#[test]
fn hydromancer_new_vote_succeed() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: make_valid_addr("zephyrus"),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            assert_eq!(
                submsg.reply_on,
                ReplyOn::Success,
                "all lock messages should be reply_on_success"
            );

            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Vote {
        tranche_id,
        proposals_votes,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(proposals_votes.len(), 1);
        assert_eq!(proposals_votes[0].proposal_id, 1);
        assert_eq!(proposals_votes[0].lock_ids, vec![0]);
    } else {
        panic!("Le message ne correspond pas au pattern attendu !");
    }

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: false,
        steerer_id: default_hydromancer_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let _ = handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

    let vessels_to_harbor =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor.len(), 1);
    assert_eq!(vessels_to_harbor[0].1.hydro_lock_id, 0);
    assert_eq!(vessels_to_harbor[0].1.steerer_id, default_hydromancer_id);
}

#[test]
fn hydromancer_change_existing_vote_succeed() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    state::insert_new_user(deps.as_mut().storage, alice_address.clone()).expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    // Simulate hydromancer vote with vessel
    let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 2,
            vessel_ids: vec![0],
        }],
    };
    let hydromancer =
        state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id).unwrap();

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: hydromancer.address.clone(),
            funds: vec![],
        },
        msg_vote_hydromancer,
    );
    assert!(result.is_ok());
    let result = result.unwrap();

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: false,
        steerer_id: default_hydromancer_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 2,
                vessel_ids: vec![0],
            }
        }],
    };

    let _ = handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

    assert_eq!(result.messages.len(), 1);
    let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![0],
        }],
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: hydromancer.address.clone(),
            funds: vec![],
        },
        msg_vote_hydromancer,
    );
    assert!(result.is_ok());
    let decoded_submessages: Vec<HydroExecuteMsg> = result
        .unwrap()
        .messages
        .iter()
        .map(|submsg| {
            assert_eq!(
                submsg.reply_on,
                ReplyOn::Success,
                "all lock messages should be reply_on_success"
            );

            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Vote {
        tranche_id,
        proposals_votes,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(proposals_votes.len(), 1);
        assert_eq!(proposals_votes[0].proposal_id, 1);
        assert_eq!(proposals_votes[0].lock_ids, vec![0]);
    } else {
        panic!("Le message ne correspond pas au pattern attendu !");
    }

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: false,
        steerer_id: default_hydromancer_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let _ = handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

    let vessels_to_harbor1 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor1.len(), 1);
    assert_eq!(vessels_to_harbor1[0].1.hydro_lock_id, 0);
    assert_eq!(vessels_to_harbor1[0].1.steerer_id, default_hydromancer_id);

    let vessels_to_harbor2 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor2.len(), 0);
}

#[test]
fn hydromancer_vote_fails_if_duplicate_vessel_id() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![1, 2],
                }
            },
            {
                VesselsToHarbor {
                    harbor_id: 2,
                    vessel_ids: vec![2, 4],
                }
            },
        ],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![]
            },
            msg,
        )
        .unwrap_err(),
        ContractError::DuplicateVesselId { vessel_id: 2 }
    );
}

#[test]
fn hydromancer_vote_fails_if_duplicate_harbor() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let msg = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![1, 2],
                }
            },
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![3, 4],
                }
            },
        ],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![]
            },
            msg,
        )
        .unwrap_err(),
        ContractError::DuplicateHarborId { harbor_id: 1 }
    );
}

//TESTS USER VOTE
#[test]
fn user_vote_fails_not_zephyrus_user() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let alice_address = make_valid_addr("alice");
    let msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![1, 2],
                }
            },
            {
                VesselsToHarbor {
                    harbor_id: 2,
                    vessel_ids: vec![3, 4],
                }
            },
        ],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: alice_address.clone(),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::UserNotFound {
            identifier: alice_address.to_string()
        }
    );
}

#[test]
fn user_vote_with_other_vessels_fail() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let alice_user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");

    let bob_address = make_valid_addr("bob");
    state::insert_new_user(deps.as_mut().storage, bob_address.clone()).expect("Should add user");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: alice_user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    let msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: bob_address.clone(),
            funds: vec![],
        },
        msg,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn user_new_vote_succeed() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    let take_control_msg = ExecuteMsg::TakeControl {
        vessel_ids: vec![0],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        take_control_msg,
    );
    assert!(result.is_ok());

    let msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            assert_eq!(
                submsg.reply_on,
                ReplyOn::Success,
                "all lock messages should be reply_on_success"
            );

            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Vote {
        tranche_id,
        proposals_votes,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(proposals_votes.len(), 1);
        assert_eq!(proposals_votes[0].proposal_id, 1);
        assert_eq!(proposals_votes[0].lock_ids, vec![0]);
    } else {
        panic!("Le message ne correspond pas au pattern attendu !");
    }

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: true,
        steerer_id: user_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };
    let _ = handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

    let vessels_to_harbor =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor.len(), 1);
    assert!(vessels_to_harbor[0].1.user_control);
    assert_eq!(vessels_to_harbor[0].1.hydro_lock_id, 0);
    assert_eq!(vessels_to_harbor[0].1.steerer_id, user_id);
}

#[test]
fn user_change_existing_hydromancer_vote_succeed() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let constants = state::get_constants(deps.as_mut().storage).unwrap();

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    let default_hydromancer =
        state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id).unwrap();

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    // Simulate hydromancer vote with vessel
    let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![0],
        }],
    };

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: default_hydromancer.address.clone(),
            funds: vec![],
        },
        msg_vote_hydromancer,
    );
    assert!(result.is_ok());

    let take_control_msg = ExecuteMsg::TakeControl {
        vessel_ids: vec![0],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        take_control_msg,
    );
    assert!(result.is_ok());

    let user_vote_msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![0],
        }],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        user_vote_msg,
    );
    assert!(res.is_ok());
    let res = res.unwrap();
    assert_eq!(res.messages.len(), 1);

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .filter(|submsg| submsg.reply_on == ReplyOn::Success)
        .map(|submsg| {
            assert_eq!(
                submsg.reply_on,
                ReplyOn::Success,
                "all lock messages should be reply_on_success"
            );

            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Vote {
        tranche_id,
        proposals_votes,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(proposals_votes.len(), 1);
        assert_eq!(proposals_votes[0].proposal_id, 1);
        assert_eq!(proposals_votes[0].lock_ids, vec![0]);
    } else {
        panic!("Le message ne correspond pas au pattern attendu !");
    }
    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: 1,
        user_vote: true,
        steerer_id: user_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };
    let _ = handle_vote_reply(deps.as_mut(), payload, vec![]).unwrap();

    let vessels_to_harbor1 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 1)
            .expect("Vessel to harbor should exist");
    assert_eq!(vessels_to_harbor1.len(), 1);
    assert!(vessels_to_harbor1[0].1.user_control);
    assert_eq!(vessels_to_harbor1[0].1.hydro_lock_id, 0);
    assert_eq!(vessels_to_harbor1[0].1.steerer_id, user_id);

    let vessels_to_harbor2 =
        state::get_vessel_to_harbor_by_harbor_id(deps.as_mut().storage, 1, 1, 2)
            .expect("Should return empty list");
    assert_eq!(vessels_to_harbor2.len(), 0);
}

#[test]
fn user_vote_fails_if_duplicate_vessel_id() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![1, 2],
                }
            },
            {
                VesselsToHarbor {
                    harbor_id: 2,
                    vessel_ids: vec![2, 4],
                }
            },
        ],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::DuplicateVesselId { vessel_id: 2 }
    );
}

#[test]
fn user_vote_fails_if_duplicate_harbor() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![1, 2],
                }
            },
            {
                VesselsToHarbor {
                    harbor_id: 1,
                    vessel_ids: vec![3, 4],
                }
            },
        ],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("zephyrus"),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::DuplicateHarborId { harbor_id: 1 }
    );
}

#[test]
fn change_hydromancer_for_unexisting_vessel_fail() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: 1,
        hydro_lock_ids: vec![0],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("alice"),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::Unauthorized {}
    );
}

#[test]
fn change_hydromancer_fail_bad_user() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: 1,
        hydro_lock_ids: vec![0],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: make_valid_addr("bob"),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::Unauthorized {}
    );
}

#[test]
fn change_hydromancer_2_vessels_with_1_fail_bad_user() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let bob_address = make_valid_addr("bob");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");
    let bob_id = state::insert_new_user(deps.as_mut().storage, bob_address.clone())
        .expect("Should add user");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 1,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: bob_id,
        },
        &bob_address,
    )
    .expect("Should add vessel");

    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: 1,
        hydro_lock_ids: vec![0, 1],
    };

    assert_eq!(
        execute(
            deps.as_mut(),
            mock_env(),
            MessageInfo {
                sender: bob_address.clone(),
                funds: vec![]
            },
            msg
        )
        .unwrap_err(),
        ContractError::Unauthorized {}
    );
}

#[test]
fn change_hydromancer_1_vessels_hydromancer_success() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let alice_user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should add user");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;
    state::add_vessel(
        deps.as_mut().storage,
        &Vessel {
            hydro_lock_id: 0,
            tokenized_share_record_id: Some(0),
            class_period: 12_000_000, // 12 lock_epoch_length
            auto_maintenance: true,
            hydromancer_id: Some(default_hydromancer_id),
            owner_id: alice_user_id,
        },
        &alice_address,
    )
    .expect("Should add vessel");

    let bob_address = make_valid_addr("bob");
    let new_hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        bob_address.clone(),
        "BOB".to_string(),
        Decimal::zero(),
    )
    .expect("Hydromancer should be added!");

    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: new_hydromancer_id,
        hydro_lock_ids: vec![0],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    //test if messages is correct and type Unvote

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Unvote {
        tranche_id,
        lock_ids,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(lock_ids.len(), 1);
        assert_eq!(lock_ids[0], 0);
    } else {
        panic!("Message is not message that it should be !");
    }

    let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
    assert_eq!(vessel.hydromancer_id.unwrap(), new_hydromancer_id);
}

#[test]
fn change_hydromancer_1_vessels_already_vote_success() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");

    state::insert_new_user(deps.as_mut().storage, alice_address.clone()).expect("Should add user");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    // Simulate hydromancer vote with vessel
    let msg_vote_hydromancer = ExecuteMsg::HydromancerVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![0],
        }],
    };
    let hydromancer =
        state::get_hydromancer(deps.as_mut().storage, constants.default_hydromancer_id).unwrap();

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: hydromancer.address.clone(),
            funds: vec![],
        },
        msg_vote_hydromancer,
    );
    assert!(result.is_ok());

    let bob_address = make_valid_addr("bob");
    let new_hydromancer_id = state::insert_new_hydromancer(
        deps.as_mut().storage,
        bob_address.clone(),
        "BOB".to_string(),
        Decimal::zero(),
    )
    .expect("Hydromance should be added !");

    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: new_hydromancer_id,
        hydro_lock_ids: vec![0],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    //test if messages is correct and type Unvote

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Unvote {
        tranche_id,
        lock_ids,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(lock_ids.len(), 1);
        assert_eq!(lock_ids[0], 0);
    } else {
        panic!("Message is not message that it should be !");
    }

    let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
    assert_eq!(vessel.hydromancer_id.unwrap(), new_hydromancer_id);

    assert!(
        state::get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1)
            .unwrap()
            .is_empty()
    );
}
// Step 1: Create vessel with hydromancer
// Step 2: Take control of vessel
// Step 3: User Vote for a proposal
// Step 4: Handle vote reply
// Step 5: Affect default hydromancer to vessel (Change hydromancer)
// Step 6: Check that the proposal time weighted shares are correct and hydromancer tws are correct

#[test]
fn change_hydromancer_vessel_already_vote_under_user_control_success() {
    let mut deps = mock_dependencies();

    init_contract(deps.as_mut());
    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should create user id");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    // Step 1: Create vessel with hydromancer
    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    // Step 2: User take control of vessel
    let take_control_msg = ExecuteMsg::TakeControl {
        vessel_ids: vec![0],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        take_control_msg,
    );
    assert!(result.is_ok());

    // Step 3: User Vote for a proposal
    let user_vote_msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: 1,
            vessel_ids: vec![0],
        }],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        user_vote_msg,
    );
    assert!(res.is_ok());

    let proposal_id = 1;

    // Step 4: Handle vote reply
    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: deps.querier.get_current_round(),
        user_vote: true,
        steerer_id: user_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: proposal_id,
                vessel_ids: vec![0],
            }
        }],
    };
    let skipped_ids = vec![];
    let result = handle_vote_reply(deps.as_mut(), payload, skipped_ids);
    assert!(result.is_ok());

    // Step 5: Affect default hydromancer to vessel (Change hydromancer)
    let msg = ExecuteMsg::ChangeHydromancer {
        tranche_id: 1,
        hydromancer_id: default_hydromancer_id,
        hydro_lock_ids: vec![0],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        msg,
    )
    .unwrap();

    //test if messages is correct and type Unvote

    let decoded_submessages: Vec<HydroExecuteMsg> = res
        .messages
        .iter()
        .map(|submsg| {
            let CosmosMsg::Wasm(WasmMsg::Execute { msg, funds, .. }) = &submsg.msg else {
                panic!("unexpected msg: {submsg:?}");
            };

            assert_eq!(funds.len(), 0, "vote on hydro does not required funds");

            from_json(msg.clone()).unwrap()
        })
        .collect();

    if let [HydroExecuteMsg::Unvote {
        tranche_id,
        lock_ids,
    }] = decoded_submessages.as_slice()
    {
        assert_eq!(*tranche_id, 1);
        assert_eq!(lock_ids.len(), 1);
        assert_eq!(lock_ids[0], 0);
    } else {
        panic!("Message is not message that it should be !");
    }
    // Step 6: Check that the proposal time weighted shares, vessel tws and hydromancer tws are correct
    let hydromancer_tws = state::get_hydromancer_time_weighted_shares_by_round(
        deps.as_ref().storage,
        deps.querier.get_current_round(),
        default_hydromancer_id,
    )
    .expect("Should get hydromancer tws even if there's no tws an empty list should be returned");
    let lockup_shares = query_hydro_lockups_shares(&deps.as_ref(), &constants, vec![0]);
    assert!(lockup_shares.is_ok());
    let lockup_shares = lockup_shares.unwrap().lockups_shares_info[0].clone();
    assert_eq!(hydromancer_tws[0].0 .0, lockup_shares.locked_rounds);
    assert_eq!(hydromancer_tws[0].0 .0, lockup_shares.locked_rounds);
    let vessel = state::get_vessel(deps.as_ref().storage, 0).expect("Vessel should exist !");
    assert!(!vessel.is_under_user_control()); // vessel should be under hydromancer control now
    assert_eq!(vessel.hydromancer_id.unwrap(), default_hydromancer_id);

    assert!(
        state::get_vessel_to_harbor_by_harbor_id(deps.as_ref().storage, 1, 1, 1)
            .unwrap()
            .is_empty()
    );
    assert!(!state::is_vessel_used_under_user_control(
        deps.as_ref().storage,
        1,
        1,
        0
    ));

    let vessel_shares =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 0);
    assert!(vessel_shares.is_ok());

    let vessel_shares_info =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 0);
    assert!(vessel_shares_info.is_ok());
    assert_eq!(
        vessel_shares_info.unwrap().time_weighted_shares,
        lockup_shares.time_weighted_shares.u128()
    );

    // check tws for hydromancer is 0
    let hydromancer_tws = state::get_hydromancer_time_weighted_shares_by_round(
        deps.as_ref().storage,
        deps.querier.get_current_round(),
        default_hydromancer_id,
    )
    .expect("Should get hydromancer tws even if there's no tws an empty list should be returned");
    assert_eq!(hydromancer_tws.len(), 1);
    assert_eq!(
        hydromancer_tws[0].1,
        lockup_shares.time_weighted_shares.u128()
    );
    assert_eq!(hydromancer_tws[0].0 .0, lockup_shares.locked_rounds);
    assert_eq!(hydromancer_tws[0].0 .1, lockup_shares.token_group_id);

    let proposal_tws = state::get_proposal_time_weighted_shares(deps.as_ref().storage, proposal_id)
        .expect("Should get proposal tws");
    assert_eq!(proposal_tws.len(), 1);
    assert_eq!(proposal_tws[0].1, 0); // user vote should have been removed so tws should be 0
    assert_eq!(proposal_tws[0].0, lockup_shares.token_group_id);
}

// Step 1: Create vessel with hydromancer
// Step 2: Simulate new round
// Step 3: Take control of vessel
// Step 4: Vote for a proposal
// Step 5: Handle vote reply
// Step 6: Check that the proposal time weighted shares are correct

#[test]
fn user_take_control_after_new_round_succeed() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let constants = state::get_constants(deps.as_mut().storage).unwrap();

    let alice_address = make_valid_addr("alice");
    let user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("User id should be created");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    let vessel_shares =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 0);
    assert!(vessel_shares.is_ok());

    // Simulate new round
    deps.querier.increment_current_round();

    let take_control_msg = ExecuteMsg::TakeControl {
        vessel_ids: vec![0],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        take_control_msg,
    );
    assert!(result.is_ok());
    let proposal_id = 1;
    let user_vote_msg = ExecuteMsg::UserVote {
        tranche_id: 1,
        vessels_harbors: vec![VesselsToHarbor {
            harbor_id: proposal_id,
            vessel_ids: vec![0],
        }],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        user_vote_msg,
    );
    assert!(result.is_ok());

    let payload = VoteReplyPayload {
        tranche_id: 1,
        round_id: deps.querier.get_current_round(),
        user_vote: true,
        steerer_id: user_id,
        vessels_harbors: vec![{
            VesselsToHarbor {
                harbor_id: 1,
                vessel_ids: vec![0],
            }
        }],
    };
    let skipped_ids = vec![];
    let result = handle_vote_reply(deps.as_mut(), payload, skipped_ids);
    assert!(result.is_ok());
    let vessel_shares =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 0);
    assert!(vessel_shares.is_ok());

    let lockup_shares = query_hydro_lockups_shares(&deps.as_ref(), &constants, vec![0]);
    assert!(lockup_shares.is_ok());
    let lockup_shares = lockup_shares.unwrap().lockups_shares_info[0].clone();

    // check tws for hydromancer is 0
    let hydromancer_tws = state::get_hydromancer_time_weighted_shares_by_round(
        deps.as_ref().storage,
        deps.querier.get_current_round(),
        default_hydromancer_id,
    )
    .expect("Should get hydromancer tws even if there's no tws an empty list should be returned");
    assert!(hydromancer_tws.is_empty());

    let hydromancer_proposal_tws = state::get_hydromancer_proposal_time_weighted_shares(
        deps.as_ref().storage,
        proposal_id,
        default_hydromancer_id,
    )
    .expect("Should get hydromancer proposal tws even if there's no tws an empty list should be returned");
    assert!(hydromancer_proposal_tws.is_empty());

    let proposal_tws = state::get_proposal_time_weighted_shares(deps.as_ref().storage, proposal_id)
        .expect("Should get proposal tws");
    assert_eq!(proposal_tws.len(), 1);
    assert_eq!(proposal_tws[0].1, lockup_shares.time_weighted_shares.u128());
    assert_eq!(proposal_tws[0].0, lockup_shares.token_group_id);
}

#[test]

// Step 1: Create 2 vessels with auto_maintenance true
// Step 2: Simulate new round
// Step 3: Auto maintain vessel
// Step 4: Check that the vessel time weighted shares for the new round are correct
fn auto_maintain_after_new_round_succeed() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000, // 3 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "1".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 1_000_000, // 1 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    deps.querier.increment_current_round();

    let auto_maintain_msg = ExecuteMsg::AutoMaintain {
        start_from_vessel_id: Some(0),
        limit: None,
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        auto_maintain_msg,
    );
    assert!(result.is_ok());

    let current_round_id = deps.querier.get_current_round();
    let result = handle_refresh_time_weighted_shares_reply(
        deps.as_mut(),
        RefreshTimeWeightedSharesReplyPayload {
            vessel_ids: vec![0],
            target_class_period: 3_000_000,
            current_round_id,
        },
    );
    assert!(result.is_ok());
    let result = handle_refresh_time_weighted_shares_reply(
        deps.as_mut(),
        RefreshTimeWeightedSharesReplyPayload {
            vessel_ids: vec![1],
            target_class_period: 1_000_000,
            current_round_id,
        },
    );
    assert!(result.is_ok());

    let vessel_0_shares =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 0);
    assert!(vessel_0_shares.is_ok());

    let vessel_1_shares =
        state::get_vessel_shares_info(deps.as_ref().storage, deps.querier.get_current_round(), 1);
    assert!(vessel_1_shares.is_ok());

    assert_eq!(vessel_0_shares.unwrap().time_weighted_shares, 1000u128);
    assert_eq!(vessel_1_shares.unwrap().time_weighted_shares, 1100u128);

    let hydromancer_tws = state::get_hydromancer_time_weighted_shares_by_round(
        deps.as_ref().storage,
        deps.querier.get_current_round(),
        default_hydromancer_id,
    )
    .expect("Should get hydromancer tws even if there's no tws an empty list should be returned");
    println!("hydromancer_tws: {:?}", hydromancer_tws);
    let vessel_0_tws = hydromancer_tws
        .iter()
        .find(|tws| tws.0 .1 == "dAtom")
        .unwrap();
    let vessel_1_tws = hydromancer_tws
        .iter()
        .find(|tws| tws.0 .1 == "stAtom")
        .unwrap();
    assert_eq!(hydromancer_tws.len(), 2);
    assert_eq!(vessel_0_tws.1, 1000u128);
    assert_eq!(vessel_1_tws.1, 1100u128);
    assert_eq!(vessel_0_tws.0 .0, 1);
    assert_eq!(vessel_1_tws.0 .0, 1);
}

#[test]
fn decommission_vessels_succeed() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 1_000_000, // 1 lock_epoch_length
        })
        .unwrap(),
    });
    // Create a vessel simulating the nft reveive
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    let decommission_msg = ExecuteMsg::DecommissionVessels {
        hydro_lock_ids: vec![0],
    };
    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        decommission_msg,
    );
    assert!(result.is_ok());
}

#[test]
fn claim_rewards_fail_unauthorized_vessel() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let _bob_address = make_valid_addr("bob");

    // Create user but don't give them any vessels
    let _user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should create user id");

    // Try to claim rewards for a vessel that doesn't exist
    let claim_msg = ExecuteMsg::Claim {
        round_id: deps.querier.get_current_round(),
        tranche_id: 1,
        vessel_ids: vec![999], // Non-existent vessel
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: alice_address.clone(),
            funds: vec![],
        },
        claim_msg,
    );

    // Should fail because user doesn't own the vessel
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn claim_rewards_fail_wrong_owner() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let constants = state::get_constants(deps.as_mut().storage).unwrap();
    let alice_address = make_valid_addr("alice");
    let bob_address = make_valid_addr("bob");

    // Create both users
    let _alice_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should create user id");
    let _bob_id = state::insert_new_user(deps.as_mut().storage, bob_address.clone())
        .expect("Should create user id");

    let default_hydromancer_id = state::get_constants(deps.as_mut().storage)
        .unwrap()
        .default_hydromancer_id;

    // Create vessel owned by Alice
    let receive_msg = ExecuteMsg::ReceiveNft(zephyrus_core::msgs::Cw721ReceiveMsg {
        sender: alice_address.to_string(),
        token_id: "0".to_string(),
        msg: to_json_binary(&VesselInfo {
            owner: alice_address.to_string(),
            auto_maintenance: true,
            hydromancer_id: default_hydromancer_id,
            class_period: 3_000_000,
        })
        .unwrap(),
    });

    let result = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: constants.hydro_config.hydro_contract_address.clone(),
            funds: vec![],
        },
        receive_msg,
    );
    assert!(result.is_ok());

    // Bob tries to claim rewards for Alice's vessel
    let claim_msg = ExecuteMsg::Claim {
        round_id: deps.querier.get_current_round(),
        tranche_id: 1,
        vessel_ids: vec![0],
    };

    let res = execute(
        deps.as_mut(),
        mock_env(),
        MessageInfo {
            sender: bob_address.clone(),
            funds: vec![],
        },
        claim_msg,
    );

    // Should fail because Bob doesn't own the vessel
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ContractError::Unauthorized);
}

#[test]
fn handle_claim_tribute_reply_insufficient_balance() {
    let mut deps = mock_dependencies();
    init_contract(deps.as_mut());

    let alice_address = make_valid_addr("alice");
    let _user_id = state::insert_new_user(deps.as_mut().storage, alice_address.clone())
        .expect("Should create user id");

    // Create payload with incorrect balance (amount + balance_before_claim doesn't match actual balance)
    let payload = ClaimTributeReplyPayload {
        proposal_id: 1,
        tribute_id: 1,
        round_id: deps.querier.get_current_round(),
        tranche_id: 1,
        amount: Coin::new(1000u128, "uatom"),
        balance_before_claim: Coin::new(500u128, "uatom"), // This would expect 1500 total
        vessels_owner: alice_address.clone(),
        vessel_ids: vec![0],
    };

    // Test handle_claim_tribute_reply with insufficient balance
    let res = handle_claim_tribute_reply(deps.as_mut(), mock_env(), payload);

    // Should fail due to insufficient tribute received
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        ContractError::InsufficientTributeReceived { tribute_id: 1 }
    );
}
