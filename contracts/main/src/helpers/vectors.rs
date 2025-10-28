use cosmwasm_std::{Coin, Uint128};
use std::collections::BTreeMap;

// This function will take hydro_unlocked_tokens (returned by Hydro contract) and received_coins (actual coins received obtained by bank balance diff)
pub fn compare_coin_vectors(hydro_unlocked_tokens: Vec<Coin>, received_coins: Vec<Coin>) -> bool {
    // First, consolidate hydro_unlocked_tokens by summing amounts for same denoms
    let mut consolidated_hydro: BTreeMap<String, Uint128> = BTreeMap::new();
    for coin in hydro_unlocked_tokens {
        *consolidated_hydro.entry(coin.denom).or_default() += coin.amount;
    }

    // Convert received_coins to BTreeMap for comparison
    // Note: We assume received_coins has unique denoms
    let received_map: BTreeMap<String, Uint128> = received_coins
        .into_iter()
        .map(|coin| (coin.denom, coin.amount))
        .collect();

    // Compare the maps
    consolidated_hydro == received_map
}

// Function to compare two Vec<u64>. There should be no duplicates in the vectors, or they should be in both.
pub fn compare_u64_vectors(mut vec1: Vec<u64>, mut vec2: Vec<u64>) -> bool {
    // First check if lengths are different
    if vec1.len() != vec2.len() {
        return false;
    }

    // Sort both vectors in-place
    vec1.sort_unstable();
    vec2.sort_unstable();

    // Compare the sorted vectors
    vec1 == vec2
}

pub fn join_u64_ids<I>(ids: I) -> String
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    ids.into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
