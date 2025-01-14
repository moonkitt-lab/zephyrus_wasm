use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo,
    Reply, Response as CwResponse, StdError, StdResult, Storage, SubMsg, WasmMsg,
};
use hydro_interface::msgs::ExecuteMsg::{LockTokens, RefreshLockDuration};
use neutron_sdk::bindings::msg::NeutronMsg;
use serde::{Deserialize, Serialize};
use zephyrus_core::msgs::{
    ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, VesselCreationMsg, VesselsResponse,
    VotingPowerResponse,
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
    state::init_pause_contract_value(deps.storage)?;
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
    vessels: Vec<VesselCreationMsg>,
    receiver: Option<String>,
) -> Result<Response, ContractError> {
    validate_contract_is_not_paused(deps.storage)?;
    let hydro_config = state::get_hydro_config(deps.storage)?;
    let mut sub_messages = vec![];
    if info.funds.len() != 1 {
        return Err(ContractError::Std(StdError::generic_err(
            "Must provide exactly one coin to lock",
        )));
    }

    let receiver = receiver.map_or(Ok::<Addr, ContractError>(info.sender.clone()), |a| {
        Ok(deps.api.addr_validate(&a)?)
    })?;

    let funds = info.funds[0].clone();
    let mut rest = funds.amount;
    let mut total_shares = 0u8;
    for (i, vessel) in vessels.iter().enumerate() {
        let hydromancer_id = vessel.hydromancer_id;
        let lock_duration = vessel.lock_duration;
        let auto_maintenance = vessel.auto_maintenance;
        total_shares += vessel.share;
        let lock_tokens_msg = LockTokens { lock_duration };
        let lsm_amount;
        if i == vessels.len() - 1 {
            lsm_amount = rest;
        } else {
            lsm_amount = Decimal::from_ratio(vessel.share, 100u128)
                .checked_mul(Decimal::from_atomics(funds.amount, 0).unwrap())
                .unwrap()
                .to_uint_floor();
            rest = rest.checked_sub(lsm_amount).unwrap();
        }
        let vessel_fund = cosmwasm_std::Coin {
            amount: lsm_amount,
            denom: funds.denom.clone(),
        };
        let execute_lock_tokens_msg = WasmMsg::Execute {
            contract_addr: hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&lock_tokens_msg)?,
            funds: vec![vessel_fund.clone()],
        };

        let build_vessel_params = BuildVesselParameters {
            lock_duration,
            auto_maintenance,
            hydromancer_id,
            owner: receiver.clone(),
        };
        let execute_lock_tokens_submsg: SubMsg<NeutronMsg> =
            SubMsg::reply_on_success(execute_lock_tokens_msg, HYDRO_LOCK_TOKENS_REPLY_ID)
                .with_payload(to_json_binary(&build_vessel_params)?);
        sub_messages.push(execute_lock_tokens_submsg);
    }
    if total_shares != 100 {
        return Err(ContractError::TotalSharesError { total_shares });
    }

    Ok(Response::new().add_submessages(sub_messages))
}

// This function loops through all the vessels, and filters those who have auto_maintenance true
// Then, it combines them by hydro_lock_duration, and calls execute_update_vessels_class
fn execute_auto_maintain(deps: DepsMut, _info: MessageInfo) -> Result<Response, ContractError> {
    validate_contract_is_not_paused(deps.storage)?;
    let vessels_ids_by_hydro_lock_duration = state::get_vessels_id_by_class()?;

    let iterator = vessels_ids_by_hydro_lock_duration.range(
        deps.storage,
        None,
        None,
        cosmwasm_std::Order::Ascending,
    );
    let mut response = Response::new();
    let hydro_config = state::get_hydro_config(deps.storage)?;
    let mut messages_counter = 0;
    // Collect all keys into a Vec<u64>
    for item in iterator {
        let (hydro_period, hydro_lock_ids) = item?;

        if hydro_lock_ids.len() == 0 {
            continue;
        }
        messages_counter += 1;
        let refresh_duration_msg = RefreshLockDuration {
            lock_ids: hydro_lock_ids.iter().cloned().collect(),
            lock_duration: hydro_period,
        };
        let execute_refresh_msg = WasmMsg::Execute {
            contract_addr: hydro_config.hydro_contract_address.to_string(),
            msg: to_json_binary(&refresh_duration_msg)?,
            funds: vec![],
        };

        response = response
            .add_attribute("Action", "Refresh lock duration")
            .add_attribute(
                ["ids ", &hydro_period.to_string()].concat(),
                hydro_lock_ids
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        response = response.add_message(execute_refresh_msg);
    }
    if messages_counter == 0 {
        return Err(ContractError::NoVesselsToAutoMaintain {});
    }

    Ok(response)
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
    validate_contract_is_not_paused(deps.storage)?;
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

fn execute_modify_auto_maintenance(
    deps: DepsMut,
    info: MessageInfo,
    hydro_lock_ids: Vec<u64>,
    auto_maintenance: bool,
) -> Result<Response, ContractError> {
    validate_contract_is_not_paused(deps.storage)?;
    for hydro_lock_id in hydro_lock_ids.iter() {
        if !state::is_vessel_owner(deps.storage, &info.sender, *hydro_lock_id)? {
            return Err(ContractError::Unauthorized {});
        }
        state::modify_auto_maintenance(deps.storage, *hydro_lock_id, auto_maintenance)?;
    }

    Ok(Response::new()
        .add_attribute("action", "modify_auto_maintenance")
        .add_attribute("new_auto_maintenance", auto_maintenance.to_string())
        .add_attribute(
            "hydro_lock_id",
            hydro_lock_ids
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ))
}

fn execute_pause_contract(
    storage: &mut dyn Storage,
    sender: &Addr,
) -> Result<Response, ContractError> {
    validate_admin_address(storage, sender)?;
    state::pause_contract(storage)?;
    Ok(Response::new().add_attribute("action", "pause_contract"))
}

fn execute_unpause_contract(
    storage: &mut dyn Storage,
    sender: &Addr,
) -> Result<Response, ContractError> {
    validate_admin_address(storage, sender)?;
    state::unpause_contract(storage)?;
    Ok(Response::new().add_attribute("action", "unpause_contract"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
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
        ExecuteMsg::ModifyAutoMaintenance {
            hydro_lock_ids,
            auto_maintenance,
        } => execute_modify_auto_maintenance(deps, info, hydro_lock_ids, auto_maintenance),
        ExecuteMsg::PauseContract {} => execute_pause_contract(deps.storage, &info.sender),
        ExecuteMsg::UnpauseContract {} => execute_unpause_contract(deps.storage, &info.sender),
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
        QueryMsg::IsContractPaused {} => {
            let paused = state::is_contract_paused(deps.storage)?;
            to_json_binary(&paused)
        }
    }
}

fn validate_contract_is_not_paused(storage: &dyn Storage) -> Result<(), ContractError> {
    let paused = state::is_contract_paused(storage)?;
    match paused {
        true => Err(ContractError::Paused),
        false => Ok(()),
    }
}

fn validate_admin_address(storage: &dyn Storage, sender: &Addr) -> Result<(), ContractError> {
    let whitelisted = state::is_whitelisted_admin(storage, sender)?;
    match whitelisted {
        true => Ok(()),
        false => Err(ContractError::Unauthorized {}),
    }
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
