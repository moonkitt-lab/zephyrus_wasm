use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply,
    Response as CwResponse, StdError, SubMsg, WasmMsg,
};
use hydro_interface::msgs::ExecuteMsg::{LockTokens, RefreshLockDuration};
use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, VesselsResponse, VotingPowerResponse,
};

use crate::{
    domain,
    errors::ContractError,
    state::{self},
};

type Response = CwResponse<NeutronMsg>;

const HYDRO_LOCK_TOKENS_REPLY_ID: u64 = 1;
const MAX_PAGINATION_LIMIT: usize = 1000;
const DEFAULT_PAGINATION_LIMIT: usize = 100;

#[derive(Serialize, Deserialize)]
struct BuildVesselParameters {
    lock_duration: u64,
    auto_maintenance: bool,
    hydromancer_id: u64,
    owner: Addr,
}

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

fn execute_build_vessel(
    deps: DepsMut,
    info: MessageInfo,
    lock_duration: u64,
    auto_maintenance: bool,
    hydromancer_id: u64,
) -> Result<Response, StdError> {
    let hydro_config = state::get_hydro_config(deps.storage)?;

    let lock_tokens_msg = LockTokens { lock_duration };
    let execute_lock_tokens_msg = WasmMsg::Execute {
        contract_addr: hydro_config.hydro_contract_address.to_string(),
        msg: to_json_binary(&lock_tokens_msg)?,
        funds: info.funds.clone(),
    };
    let build_vessel_params = BuildVesselParameters {
        lock_duration,
        auto_maintenance,
        hydromancer_id,
        owner: info.sender.clone(),
    };
    let execute_lock_tokens_submsg: SubMsg<NeutronMsg> =
        SubMsg::reply_on_success(execute_lock_tokens_msg, HYDRO_LOCK_TOKENS_REPLY_ID)
            .with_payload(to_json_binary(&build_vessel_params)?);

    Ok(Response::new().add_submessage(execute_lock_tokens_submsg))
}

// This function loops through all the vessels, and filters those who have auto_maintenance true
// Then, it combines them by hydro_lock_duration, and calls execute_update_vessels_class
fn execute_auto_maintain(_deps: DepsMut, _info: MessageInfo) -> Result<Response, StdError> {
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
) -> Result<Response, StdError> {
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

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::BuildVessel {
            lock_duration,
            auto_maintenance,
            hydromancer_id,
        } => execute_build_vessel(deps, info, lock_duration, auto_maintenance, hydromancer_id),
        ExecuteMsg::AutoMaintain {} => execute_auto_maintain(deps, info),
        ExecuteMsg::UpdateVesselsClass {
            hydro_lock_ids,
            hydro_lock_duration,
        } => execute_update_vessels_class(deps, info, hydro_lock_ids, hydro_lock_duration),
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
) -> Result<Binary, StdError> {
    let owner = deps.api.addr_validate(owner.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);
    let vessels = state::get_vessels_by_owner(deps.storage, owner, start_index, limit)?;
    let total = vessels.len();
    to_json_binary(&VesselsResponse {
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
) -> Result<Binary, StdError> {
    let hydromancer_addr = deps.api.addr_validate(hydromancer_addr.as_str())?;
    let limit = limit
        .unwrap_or(DEFAULT_PAGINATION_LIMIT)
        .min(MAX_PAGINATION_LIMIT);
    let start_index = start_index.unwrap_or(0);
    let vessels =
        state::get_vessels_by_hydromancer(deps.storage, hydromancer_addr, start_index, limit)?;
    let total = vessels.len();
    to_json_binary(&VesselsResponse {
        vessels,
        start_index,
        limit,
        total,
    })
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, StdError> {
    let binary = match msg {
        QueryMsg::VotingPower {} => {
            query_voting_power(deps, env).and_then(|res| to_json_binary(&res))
        }
        QueryMsg::VesselsByOwner {
            owner,
            start_index,
            limit,
        } => query_vessels_by_owner(deps, owner, start_index, limit)
            .and_then(|res| to_json_binary(&res)),
        QueryMsg::VesselsByHydromancer {
            hydromancer_addr,
            start_index,
            limit,
        } => query_vessels_by_hydromancer(deps, hydromancer_addr, start_index, limit)
            .and_then(|res| to_json_binary(&res)),
    }?;

    Ok(binary)
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

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}

fn handle_lock_tokens_reply(deps: DepsMut, reply: Reply) -> Result<Response, ContractError> {
    let response = reply
        .result
        .into_result()
        .expect("always issued on_success");

    let build_vessel_params: BuildVesselParameters =
        from_json(reply.payload).expect("build vessel parameters always attached");

    let lock_id = response
        .events
        .into_iter()
        .flat_map(|e| e.attributes)
        .find_map(|attr| (attr.key == "lock_id").then(|| attr.value.parse().ok()))
        .flatten()
        .expect("lock tokens reply always contains valid lock_id attribute");
    domain::vessel::create_new_vessel(
        deps,
        lock_id,
        build_vessel_params.auto_maintenance,
        build_vessel_params.lock_duration,
        build_vessel_params.hydromancer_id,
        &build_vessel_params.owner,
    )?;

    // do something else

    Ok(Response::new()
        .add_attribute("action", "build_vessel")
        .add_attribute("hydro_lock_id", lock_id.to_string())
        .add_attribute("owner", build_vessel_params.owner.to_string()))
}
