use std::time::SystemTime;

use cosmwasm_std::{
    coin, from_json,
    testing::{MockApi, MockQuerier as StdMockQuerier, MockStorage},
    to_json_binary, Addr, Binary, ContractResult, Decimal, Empty, GrpcQuery, OwnedDeps, Querier,
    QuerierResult, QueryRequest, StdResult, SystemError, SystemResult, Timestamp, Uint128,
    WasmQuery,
};
use hydro_interface::msgs::{
    CollectionInfo, CurrentRoundResponse, HydroConstants, HydroConstantsResponse, HydroQueryMsg,
    LockEntryV2, LockEntryWithPower, LockPowerEntry, LockupWithPerTrancheInfo, LockupsSharesInfo,
    LockupsSharesResponse, PerTrancheLockupInfo, RoundLockPowerSchedule,
    SpecificUserLockupsResponse, SpecificUserLockupsWithTrancheInfosResponse, Tranche,
    TranchesResponse,
};
use neutron_std::types::ibc::applications::transfer::v1::{
    DenomTrace, QueryDenomTraceRequest, QueryDenomTraceResponse,
};
use prost::Message;

use crate::testing::make_valid_addr;

pub struct MockWasmQuerier {
    hydro_contract: String,
    current_round: u64,
    hydro_constants: Option<HydroConstants>,
    error_specific_user_lockups: bool,
}

impl MockWasmQuerier {
    pub fn new(
        hydro_contract: String,
        current_round: u64,
        hydro_constants: Option<HydroConstants>,
        error_specific_user_lockups: bool,
    ) -> Self {
        Self {
            hydro_contract,
            current_round,
            hydro_constants,
            error_specific_user_lockups,
        }
    }

    pub fn handler(&self, query: &WasmQuery) -> QuerierResult {
        match query {
            WasmQuery::Smart { contract_addr, msg } => {
                if *contract_addr != self.hydro_contract {
                    return SystemResult::Err(SystemError::NoSuchContract {
                        addr: contract_addr.to_string(),
                    });
                }

                let response = match from_json(msg).unwrap() {
                    HydroQueryMsg::CurrentRound {} => to_json_binary(&CurrentRoundResponse {
                        round_id: self.current_round,
                        round_end: Timestamp::from_seconds(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        ),
                    }),
                    HydroQueryMsg::Constants {} => to_json_binary(&HydroConstantsResponse {
                        constants: self
                            .hydro_constants
                            .clone()
                            .unwrap_or_else(|| HydroConstants {
                                round_length: 1000,
                                lock_epoch_length: 1,
                                first_round_start: Timestamp::from_seconds(1000),
                                max_locked_tokens: 50_000,
                                known_users_cap: 250,
                                paused: false,
                                max_deployment_duration: 3,
                                round_lock_power_schedule: RoundLockPowerSchedule {
                                    round_lock_power_schedule: vec![
                                        LockPowerEntry {
                                            locked_rounds: 1,
                                            power_scaling_factor: Decimal::from_ratio(
                                                10u128, 10u128,
                                            ),
                                        },
                                        LockPowerEntry {
                                            locked_rounds: 2,
                                            power_scaling_factor: Decimal::from_ratio(
                                                20u128, 10u128,
                                            ),
                                        },
                                        LockPowerEntry {
                                            locked_rounds: 3,
                                            power_scaling_factor: Decimal::from_ratio(
                                                30u128, 10u128,
                                            ),
                                        },
                                    ],
                                },
                                cw721_collection_info: CollectionInfo {
                                    name: "Test Collection".to_string(),
                                    symbol: "TEST".to_string(),
                                },
                            }),
                    }),
                    HydroQueryMsg::SpecificUserLockups { address, lock_ids } => {
                        self.handle_specific_user_lockups(&address, &lock_ids)
                    }
                    HydroQueryMsg::LockupsShares { lock_ids } => {
                        self.handle_lockups_shares(&lock_ids)
                    }
                    HydroQueryMsg::Tranches {} => to_json_binary(&TranchesResponse {
                        tranches: vec![Tranche {
                            id: 1,
                            name: "Atom".to_string(),
                            metadata: "".to_string(),
                        }],
                    }),
                    HydroQueryMsg::SpecificUserLockupsWithTrancheInfos {
                        address: _,
                        lock_ids,
                    } => self.handle_specific_user_lockups_with_tranche_infos(&lock_ids),
                };

                SystemResult::Ok(ContractResult::Ok(response.unwrap()))
            }
            _ => SystemResult::Err(SystemError::UnsupportedRequest {
                kind: "unsupported query type".to_string(),
            }),
        }
    }

    fn handle_specific_user_lockups(&self, address: &str, lock_ids: &[u64]) -> StdResult<Binary> {
        if self.error_specific_user_lockups {
            return to_json_binary(&SpecificUserLockupsResponse { lockups: vec![] });
        }

        let mut lockups_with_power: Vec<LockEntryWithPower> = vec![];
        for lock_id in lock_ids {
            lockups_with_power.push(LockEntryWithPower {
                lock_entry: LockEntryV2 {
                    lock_id: *lock_id,
                    owner: Addr::unchecked(address),
                    funds: coin(1000u128, "uatom"),
                    lock_start: Timestamp::from_seconds(1000),
                    lock_end: Timestamp::from_seconds(2000),
                },
                current_voting_power: Uint128::from(1000u128),
            });
        }
        to_json_binary(&SpecificUserLockupsResponse {
            lockups: lockups_with_power,
        })
    }

    fn handle_lockups_shares(&self, lock_ids: &[u64]) -> StdResult<Binary> {
        let mut shares_info: Vec<LockupsSharesInfo> = vec![];
        for lock_id in lock_ids {
            shares_info.push(LockupsSharesInfo {
                lock_id: *lock_id,
                time_weighted_shares: Uint128::from(1000u128),
                token_group_id: "dAtom".to_string(),
                locked_rounds: 1,
            });
        }
        to_json_binary(&LockupsSharesResponse {
            lockups_shares_info: shares_info,
        })
    }

    fn handle_specific_user_lockups_with_tranche_infos(
        &self,
        lock_ids: &[u64],
    ) -> StdResult<Binary> {
        let mut lockup_tranche_infos: Vec<LockupWithPerTrancheInfo> = vec![];
        for lock_id in lock_ids {
            let per_tranche_infos = vec![PerTrancheLockupInfo {
                tranche_id: 1,
                next_round_lockup_can_vote: 2,
                current_voted_on_proposal: None,
                tied_to_proposal: None,
                historic_voted_on_proposals: vec![],
            }];
            lockup_tranche_infos.push(LockupWithPerTrancheInfo {
                lock_with_power: LockEntryWithPower {
                    lock_entry: LockEntryV2 {
                        lock_id: *lock_id,
                        owner: make_valid_addr("owner"),
                        funds: coin(1000u128, "uatom"),
                        lock_start: Timestamp::from_seconds(1000),
                        lock_end: Timestamp::from_seconds(2000),
                    },
                    current_voting_power: Uint128::from(1000u128),
                },
                per_tranche_info: per_tranche_infos,
            });
        }
        to_json_binary(&SpecificUserLockupsWithTrancheInfosResponse {
            lockups_with_per_tranche_infos: lockup_tranche_infos,
        })
    }
}

pub struct MockQuerier {
    base: StdMockQuerier,
    wasm_querier: MockWasmQuerier,
}

impl MockQuerier {
    fn new(wasm_querier: MockWasmQuerier) -> Self {
        Self {
            base: StdMockQuerier::new(&[]),
            wasm_querier,
        }
    }
}

impl Querier for MockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(_) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: "Parsing query request".to_string(),
                    request: bin_request.into(),
                })
            }
        };

        match request {
            QueryRequest::Wasm(wasm_query) => self.wasm_querier.handler(&wasm_query),
            QueryRequest::Grpc(GrpcQuery { path, data }) => self.handle_grpc_query(&path, &data),
            _ => self.base.raw_query(bin_request),
        }
    }
}

impl MockQuerier {
    fn handle_grpc_query(&self, path: &str, data: &[u8]) -> QuerierResult {
        let contract_result: ContractResult<Binary> = match path {
            "/ibc.applications.transfer.v1.Query/DenomTrace" => {
                let QueryDenomTraceRequest { hash } = QueryDenomTraceRequest::decode(data).unwrap();

                let denom_trace = match hash.as_str() {
                    "69ED129755461CF93B7E64A277A3552582B47A64F826F05E4F43E22C2D476C02" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "cosmosvaloper18hl5c9xn5dze2g50uaw0l2mr02ew57zk0auktn/12"
                                .to_owned(),
                        }
                    }
                    "FB6F9C479D2E47419EAA9C9A48B325F68A032F76AFA04890F1278C47BC0A8BB4" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "cosmosvaloper18hl5c9xn5dze2g50uaw0l2mr02ew57zk0auktn/10"
                                .to_owned(),
                        }
                    }
                    "27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2" => {
                        DenomTrace {
                            path: "transfer/channel-0".to_owned(),
                            base_denom: "uatom".to_owned(),
                        }
                    }
                    _ => {
                        return SystemResult::Err(SystemError::InvalidRequest {
                            error: format!("Unknown denom trace hash: {}", hash),
                            request: data.into(),
                        })
                    }
                };

                ContractResult::Ok(
                    QueryDenomTraceResponse {
                        denom_trace: Some(denom_trace),
                    }
                    .encode_to_vec()
                    .into(),
                )
            }
            _ => {
                return SystemResult::Err(SystemError::UnsupportedRequest {
                    kind: format!("unsupported grpc query: {}", path),
                })
            }
        };

        SystemResult::Ok(contract_result)
    }
}

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    let hydro_addr = make_valid_addr("hydro").into_string();
    let wasm_querier = MockWasmQuerier::new(hydro_addr, 1, None, false);
    let querier = MockQuerier::new(wasm_querier);

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    }
}

pub fn mock_hydro_contract(
    deps: &mut OwnedDeps<MockStorage, MockApi, MockQuerier>,
    error_specific_user_lockups: bool,
) {
    let hydro_addr = make_valid_addr("hydro_addr").into_string();
    let wasm_querier = MockWasmQuerier::new(hydro_addr, 1, None, error_specific_user_lockups);
    deps.querier = MockQuerier::new(wasm_querier);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_querier_creation() {
        let _deps = mock_dependencies();
        // Test passes if no panic occurs
    }
}
