use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response as CwResponse,
    StdError,
};
use neutron_sdk::bindings::msg::NeutronMsg;
use zephyrus_core::msgs::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, VotingPowerResponse};

type Response = CwResponse<NeutronMsg>;

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::BuildVessel {} => todo!(),
    }
}

fn query_voting_power(_deps: Deps, _env: Env) -> Result<VotingPowerResponse, StdError> {
    todo!()
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, StdError> {
    let binary = match msg {
        QueryMsg::VotingPower {} => {
            query_voting_power(deps, env).and_then(|res| to_json_binary(&res))
        }
    }?;

    Ok(binary)
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, StdError> {
    Ok(Response::default())
}
