use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, AnyMsg, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Reply, Response as CwResponse, StdError, StdResult, SubMsg, WasmMsg,
};
use hydro_interface::{
    msgs::ExecuteMsg::{LockTokens, RefreshLockDuration},
    QuerierExt,
};
use neutron_sdk::{
    bindings::{
        msg::NeutronMsg,
        types::{Height, KVKey},
    },
    interchain_queries::{types::QueryPayload, v047::types::STAKING_STORE_KEY},
    proto_types::neutron::interchainqueries::{MsgSubmitQueryResult, QueryResult},
    sudo::msg::SudoMsg,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use zephyrus_core::{
    ibc::{DenomTrace, QuerierExt as _},
    msgs::{
        BuildVesselParams, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Vessel,
        VesselsResponse, VotingPowerResponse,
    },
    neutron::QuerierExt as _,
};

use crate::{
    errors::{ContractError, TokenOwnershipProofError},
    state::{self, VesselOwnershipState},
};

type Response = CwResponse<NeutronMsg>;

const ESCROW_ICA_NAME: &str = "escrow";
const HYDRO_LOCK_TOKENS_REPLY_ID: u64 = 1;
const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

#[derive(Serialize, Deserialize)]
struct BuildVesselOnReplyPayload {
    params: BuildVesselParams,
    tokenized_share_record_id: u64,
    owner: Addr,
}

// message TokenizeShareRecord {
//   option (gogoproto.equal) = true;

//   uint64 id             = 1;
//   string owner          = 2;
//   string module_account = 3; // module account take the role of delegator
//   string validator      = 4; // validator delegated to for tokenize share record creation
// }

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> {
    state::initialize_sequences(deps.storage)?;

    let mut whitelist_admins: Vec<Addr> = vec![];
    for admin in msg.whitelist_admins {
        let admin_addr = deps.api.addr_validate(&admin)?;
        if !whitelist_admins.contains(&admin_addr) {
            whitelist_admins.push(admin_addr.clone());
        }
    }
    state::update_whitelist_admins(deps.storage, whitelist_admins)?;
    let hydro_config = state::HydroConfig {
        hydro_contract_address: deps.api.addr_validate(&msg.hydro_contract_address)?,
        hydro_tribute_contract_address: deps.api.addr_validate(&msg.tribute_contract_address)?,
    };

    state::update_hydro_config(deps.storage, hydro_config)?;

    let hydromancer_address = deps.api.addr_validate(&msg.default_hydromancer_address)?;

    let default_hydromancer_id = state::insert_new_hydromancer(
        deps.storage,
        hydromancer_address,
        msg.default_hydromancer_name,
        msg.default_hydromancer_commission_rate,
    )?;

    state::save_default_hydroamancer_id(deps.storage, default_hydromancer_id)?;

    Ok(Response::default())
}

fn extract_tokenized_share_record_id(denom_trace: &DenomTrace) -> Option<u64> {
    denom_trace
        .base_denom
        .rsplit_once('/')
        .and_then(|(_, id_str)| id_str.parse().ok())
}

fn execute_build_vessel(
    deps: DepsMut,
    info: MessageInfo,
    vessels: Vec<BuildVesselParams>,
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    if info.funds.is_empty() {
        return Err(ContractError::NoTokensReceived);
    }

    if vessels.len() != info.funds.len() {
        return Err(ContractError::CreateVesselParamsLengthMismatch {
            params_len: vessels.len(),
            funds_len: info.funds.len(),
        });
    }

    let hydro_config = state::get_hydro_config(deps.storage)?;

    let owner = receiver
        .map(|addr| deps.api.addr_validate(&addr))
        .transpose()?
        .unwrap_or(info.sender);

    let mut hydro_lock_msgs = vec![];

    for (params, token) in vessels.into_iter().zip(info.funds) {
        if !state::hydromancer_exists(deps.storage, params.hydromancer_id) {
            return Err(ContractError::HydromancerNotFound {
                hydromancer_id: params.hydromancer_id,
            });
        }

        let denom_trace = deps.querier.ibc_denom_trace(&token.denom)?;

        let tokenized_share_record_id = extract_tokenized_share_record_id(&denom_trace)
            .ok_or_else(|| ContractError::InvalidLsmTokenRecieved(token.denom.clone()))?;

        if state::is_tokenized_share_record_active(deps.storage, tokenized_share_record_id)? {
            return Err(ContractError::TokenizedShareRecordAlreadyInActiveUse(
                tokenized_share_record_id,
            ));
        }

        let payload = to_json_binary(&BuildVesselOnReplyPayload {
            params,
            tokenized_share_record_id,
            owner: owner.clone(),
        })?;

        let contract_addr = hydro_config.hydro_contract_address.clone().into_string();

        let msg = to_json_binary(&LockTokens {
            lock_duration: params.lock_duration,
        })?;

        let lock_submsg = SubMsg::reply_on_success(
            WasmMsg::Execute {
                contract_addr,
                msg,
                funds: vec![token],
            },
            HYDRO_LOCK_TOKENS_REPLY_ID,
        )
        .with_payload(payload);

        hydro_lock_msgs.push(lock_submsg);
    }

    Ok(Response::new().add_submessages(hydro_lock_msgs))
}

// This function loops through all the vessels, and filters those who have auto_maintenance true
// Then, it combines them by hydro_lock_duration, and calls execute_update_vessels_class
fn execute_auto_maintain(_deps: DepsMut, _info: MessageInfo) -> Result<Response, ContractError> {
    todo!()
}

// This function takes a list of vessels (hydro_lock_ids) and a duration
// And calls the Hydro function:
// ExecuteMsg::RefreshLockDuration {
//     lock_ids,
//     lock_duration,
// }
// TODO: Need to be careful that all the vessels are currently less than hydro_lock_duration
// Otherwise, the RefreshLockDuration will fail
fn execute_update_vessels_class(
    deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    hydro_lock_duration: u64,
) -> Result<Response, ContractError> {
    let hydro_config = state::get_hydro_config(deps.storage)?;

    let refresh_duration_msg = RefreshLockDuration {
        lock_ids: hydro_lock_ids,
        lock_duration: hydro_lock_duration,
    };

    // There should not be any funds?
    let execute_refresh_duration_msg = WasmMsg::Execute {
        contract_addr: hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&refresh_duration_msg)?,
        funds: info.funds.clone(),
    };

    Ok(Response::new().add_message(execute_refresh_duration_msg))
}

/// Registers the ICA used to hold the tokenized share ownership.
/// If the ICA connection is closed due to IBC error, this can be executed again to re-instate it.
fn execute_register_ica(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let received_funds = cw_utils::one_coin(&info)?;

    let required_funds = deps.querier.interchain_account_register_fee()?;

    if received_funds != required_funds {
        return Err(ContractError::InsufficientIcaRegistrationFunds {
            received: received_funds,
            required: required_funds,
        });
    }

    let hydro_config = state::get_hydro_config(deps.storage)?;

    let hub_connection_id = deps
        .querier
        .hydro_hub_connection_id(&hydro_config.hydro_contract_address)?;

    let register_ica_msg = NeutronMsg::register_interchain_account(
        hub_connection_id,
        ESCROW_ICA_NAME.to_owned(),
        Some(info.funds),
    );

    Ok(Response::default().add_message(register_ica_msg))
}

#[derive(Message)]
struct TokenizedShareRecord {
    #[prost(uint64, tag = "1")]
    id: u64,
    #[prost(string, tag = "2")]
    owner: String,
    #[prost(string, tag = "3")]
    module_account: String,
    #[prost(string, tag = "4")]
    validator: String,
}

fn execute_sell_vessel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    hydro_lock_id: u64,
    query_result: Binary,
    query_height: Height,
) -> Result<Response, ContractError> {
    // check vessel is elligible for sale by the message sender
    let VesselOwnershipState::OwnedByUser { owner } =
        state::get_vessel_ownership_state(deps.storage, hydro_lock_id)?
    else {
        return Err(ContractError::VesselCannotBeSold);
    };

    if info.sender.as_str() != owner {
        return Err(ContractError::SenderIsNotVesselOwner);
    }

    // check the submitted query result and proof is from a height higher than any previously set minimum height to prevent re-use attack
    if let Some(minimum_proof_height) =
        state::get_vessel_token_ownership_proof_minimum_height(deps.storage, hydro_lock_id)?
    {
        if query_height.revision_number < minimum_proof_height.revision_number
            || query_height.revision_height < minimum_proof_height.revision_height
        {
            return Err(TokenOwnershipProofError::BelowMinimumHeight {
                received: query_height,
                minimum: minimum_proof_height,
            }
            .into());
        }
    }

    // check that the query shows the ownership has been transferred to the escrow ICA
    let mut query_result = QueryResult::decode(query_result.as_slice())?;

    if query_result.kv_results.len() != 1 {
        return Err(TokenOwnershipProofError::IncorrectKvResultsLength.into());
    }

    let Some(escrow_ica_address) = state::get_escrow_ica_address(deps.storage)? else {
        return Err(ContractError::EscrowIcaDoesNotExist);
    };

    let query_result_owner = TokenizedShareRecord::decode(
        query_result
            .kv_results
            .first()
            .expect("kv_results length == 1")
            .value
            .as_slice(),
    )?
    .owner;

    if query_result_owner != escrow_ica_address {
        return Err(TokenOwnershipProofError::OwnerDoesNotMatchIcaAddress {
            query_result_owner,
            escrow_ica_address,
        }
        .into());
    }

    // transfer ownership to the escrow ICA and update vessel ownership state
    // this will be reverted if Neutron detects the proof as invalid
    state::transfer_vessel_ownership_to_protocol(deps.storage, hydro_lock_id, &escrow_ica_address)?;

    let config = state::get_hydro_config(deps.storage)?;

    // construct message sequence
    let connection_id = deps
        .querier
        .hydro_hub_connection_id(&config.hydro_contract_address)?;

    let client_id = deps.querier.ibc_connection(&connection_id)?.client_id;

    let vessel = state::get_vessel(deps.storage, hydro_lock_id)?;

    let register_icq_msg = NeutronMsg::register_interchain_query(
        QueryPayload::KV(vec![KVKey {
            path: STAKING_STORE_KEY.to_owned(),
            key: [
                [0x81].as_slice(),
                vessel.tokenized_share_record_id.to_be_bytes().as_slice(),
            ]
            .concat()
            .into(),
        }]),
        connection_id,
        u64::MAX,
    )?;

    query_result.allow_kv_callbacks = false;

    let query_id = deps
        .querier
        .last_registered_interchain_query_id()?
        .map_or(1, |last_id| last_id + 1);

    let update_icq_msg = CosmosMsg::Any(AnyMsg {
        type_url: "/neutron.interchainqueries.MsgSubmitQueryResult".to_owned(),
        value: MsgSubmitQueryResult {
            query_id,
            sender: env.contract.address.into_string(),
            client_id,
            result: Some(query_result),
        }
        .encode_to_vec()
        .into(),
    });

    let remove_icq_msg = NeutronMsg::remove_interchain_query(query_id);

    Ok(Response::default()
        // TODO: Add message to pay sender for vessel
        .add_message(register_icq_msg)
        .add_message(update_icq_msg)
        .add_message(remove_icq_msg))
}

fn execute_buy_vessel(deps: DepsMut, hydro_lock_id: u64) -> Result<Response, ContractError> {
    todo!()
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::BuildVessel { vessels, receiver } => {
            execute_build_vessel(deps, info, vessels, receiver)
        }
        ExecuteMsg::AutoMaintain {} => execute_auto_maintain(deps, info),
        ExecuteMsg::UpdateVesselsClass {
            hydro_lock_ids,
            hydro_lock_duration,
        } => execute_update_vessels_class(deps, info, hydro_lock_ids, hydro_lock_duration),
        ExecuteMsg::RegisterIca {} => execute_register_ica(deps, info),
        ExecuteMsg::SellVessel {
            hydro_lock_id,
            query_result,
            height,
        } => execute_sell_vessel(deps, env, info, hydro_lock_id, query_result, height),
        ExecuteMsg::BuyVessel { hydro_lock_id } => execute_buy_vessel(deps, hydro_lock_id),
    }
}

fn query_voting_power(_deps: Deps, _env: Env) -> Result<VotingPowerResponse, StdError> {
    todo!()
}

fn query_vessels_by_owner(
    deps: Deps,
    owner: String,
    start_index: Option<usize>,
    limit: Option<usize>,
) -> StdResult<VesselsResponse> {
    let owner = deps.api.addr_validate(owner.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);

    let vessels = state::get_vessels_by_owner(deps.storage, owner.clone(), start_index, limit)
        .map_err(|e| {
            StdError::generic_err(format!("Failed to get vessels for {}: {}", owner, e))
        })?;

    let total = vessels.len();

    Ok(VesselsResponse {
        vessels,
        start_index,
        limit,
        total,
    })
}

fn query_vessels_by_hydromancer(
    deps: Deps,
    hydromancer_addr: String,
    start_index: Option<usize>,
    limit: Option<usize>,
) -> StdResult<VesselsResponse> {
    let hydromancer_addr = deps.api.addr_validate(hydromancer_addr.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);

    let vessels =
        state::get_vessels_by_hydromancer(deps.storage, hydromancer_addr, start_index, limit)?;
    let total = vessels.len();

    Ok(VesselsResponse {
        vessels,
        start_index,
        limit,
        total,
    })
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, StdError> {
    match msg {
        QueryMsg::VotingPower {} => to_json_binary(&query_voting_power(deps, env)?),
        QueryMsg::VesselsByOwner {
            owner,
            start_index,
            limit,
        } => to_json_binary(&query_vessels_by_owner(deps, owner, start_index, limit)?),
        QueryMsg::VesselsByHydromancer {
            hydromancer_addr,
            start_index,
            limit,
        } => to_json_binary(&query_vessels_by_hydromancer(
            deps,
            hydromancer_addr,
            start_index,
            limit,
        )?),
    }
}

fn handle_lock_tokens_reply(deps: DepsMut, reply: Reply) -> Result<Response, ContractError> {
    let response = reply
        .result
        .into_result()
        .expect("always issued on_success");

    let BuildVesselOnReplyPayload {
        params:
            BuildVesselParams {
                lock_duration,
                auto_maintenance,
                hydromancer_id,
            },
        tokenized_share_record_id,
        owner,
    } = from_json(reply.payload).expect("build vessel parameters always attached");

    let hydro_lock_id = response
        .events
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| (attr.key == "lock_id").then(|| attr.value.parse().ok()))
        .flatten()
        .expect("lock tokens reply always contains valid lock_id attribute");

    let vessel = Vessel {
        hydro_lock_id,
        class_period: lock_duration,
        tokenized_share_record_id,
        hydromancer_id,
        auto_maintenance,
    };

    state::add_vessel(deps.storage, &vessel, &owner)?;

    Ok(Response::new()
        .add_attribute("action", "build_vessel")
        .add_attribute("hydro_lock_id", hydro_lock_id.to_string())
        .add_attribute("owner", owner))
}

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        // Cas : retour du premier message
        HYDRO_LOCK_TOKENS_REPLY_ID => handle_lock_tokens_reply(deps, msg),
        _ => Err(ContractError::CustomError {
            msg: "Unknown reply id".to_string(),
        }),
    }
}

pub fn sudo_handle_open_ack(
    deps: DepsMut,
    counterparty_version: String,
) -> Result<Response, ContractError> {
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct OpenAckVersion {
        version: String,
        controller_connection_id: String,
        host_connection_id: String,
        address: String,
        encoding: String,
        tx_type: String,
    }

    let parsed_version: OpenAckVersion =
        from_json(counterparty_version).expect("valid counterparty_version");

    state::save_escrow_ica_address(deps.storage, &parsed_version.address)?;

    Ok(Response::default())
}

#[entry_point]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::OpenAck {
            counterparty_version,
            ..
        } => sudo_handle_open_ack(deps, counterparty_version),

        SudoMsg::Response { .. } => todo!("transfer ownership, record ibc client update height for remote"),

        SudoMsg::Error { .. } => todo!("allow transfer ownership retry"),

        SudoMsg::Timeout { .. } => todo!("allow transfer ownership or is it possible that the IBC callback timed out but ownership still transferred? accept proof?"),

        _ => Ok(Response::default())
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}
