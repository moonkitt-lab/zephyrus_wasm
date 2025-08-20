use cosmwasm_std::{Deps, StdResult};
use hydro_interface::msgs::{ProposalTributesResponse, Tribute, TributeQueryMsg};
use zephyrus_core::state::Constants;

pub fn query_tribute_proposal_tributes(
    deps: &Deps,
    constants: &Constants,
    round_id: u64,
    proposal_id: u64,
) -> StdResult<Vec<Tribute>> {
    let mut finished = false;
    let mut all_tributes: Vec<Tribute> = Vec::new();
    let mut start_from = 0u32;
    let limit = 100u32;

    while !finished {
        let proposal_tributes: ProposalTributesResponse = deps.querier.query_wasm_smart(
            constants
                .hydro_config
                .hydro_tribute_contract_address
                .to_string(),
            &TributeQueryMsg::ProposalTributes {
                round_id,
                proposal_id,
                start_from,
                limit,
            },
        )?;

        all_tributes.extend(proposal_tributes.tributes.clone());

        if proposal_tributes.tributes.len() < limit as usize {
            finished = true;
        }

        start_from += limit;
    }

    Ok(all_tributes)
}
