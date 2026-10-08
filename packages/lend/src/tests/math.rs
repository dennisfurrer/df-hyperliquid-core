//! The reserve math against every reserve on file:
//!
//! - `reserves.json`: live `allBorrowLendReserveStates`, mainnet and testnet,
//!   2026-10-08; `reserve.json`: live `borrowLendReserveState`, the same day;
//! - `atlas_pm.json`: atlas's fixtures, a portfolio-margin account's spot and
//!   lending as it answered on 2026-10-02, and the reserves of 2026-09-30.
//!
//! What each test expects was worked out from the recorded figures by hand.

use std::collections::BTreeMap;

use hl_core::{Dec, Network};
use serde::Deserialize;

use super::*;
use crate::state::{ReserveState, UserState};

const RESERVES: &str = include_str!("reserves.json");
const RESERVE: &str = include_str!("reserve.json");
const ATLAS: &str = include_str!("atlas_pm.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn reserves(network: Network) -> Vec<(u32, ReserveState)> {
    let all: BTreeMap<String, Vec<(u32, ReserveState)>> = serde_json::from_str(RESERVES).unwrap();
    all[network.name()].clone()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Atlas {
    spot_clearinghouse_state: Spot,
    borrow_lend_user_state: UserState,
    all_borrow_lend_reserve_states: Vec<(u32, ReserveState)>,
}

#[derive(Deserialize)]
struct Spot {
    balances: Vec<Balance>,
}

#[derive(Deserialize)]
struct Balance {
    token: u32,
    total: Dec,
    hold: Dec,
}

fn atlas() -> Atlas {
    serde_json::from_str(ATLAS).unwrap()
}

/// Every recorded reserve, from three days and both networks.
fn every_reserve() -> Vec<(String, u32, ReserveState)> {
    let mut all = Vec::new();
    for network in Network::ALL {
        all.extend(reserves(network).into_iter().map(|(token, state)| (network.name().to_owned(), token, state)));
    }
    let single: BTreeMap<String, ReserveState> = serde_json::from_str(RESERVE).unwrap();
    all.extend(single.into_iter().map(|(network, state)| (format!("{network} single"), 0, state)));
    all.extend(
        atlas().all_borrow_lend_reserve_states.into_iter().map(|(token, state)| ("atlas".to_owned(), token, state)),
    );
    all
}

/// Utilization is borrowed over supplied, and the supply rate the borrow
/// rate times utilization less the tenth the protocol keeps, both to ten
/// places: so on every reserve read.
#[test]
fn utilization_and_the_supply_rate_follow_from_the_reserve() {
    let all = every_reserve();
    assert_eq!(all.len(), 7 + 5 + 2 + 5);
    for (from, token, reserve) in all {
        assert_eq!(reserve.utilization_now(), Some(reserve.utilization), "{from} {token}: utilization");
        assert_eq!(reserve.supply_rate_now(), reserve.supply_yearly_rate, "{from} {token}: the supply rate");
        assert_eq!(borrow_rate(reserve.utilization), reserve.borrow_yearly_rate, "{from} {token}: the borrow rate");
    }
}

/// The venue's docs: 5% a year, and 4.75 more per unit of utilization past
/// 80%. No reserve on file has passed 80%, so the kink is the docs' alone.
#[test]
fn the_borrow_rate_turns_up_past_eighty_percent() {
    assert_eq!(borrow_rate(Dec::ZERO), dec("0.05"));
    assert_eq!(borrow_rate(dec("0.8")), dec("0.05"));
    assert_eq!(borrow_rate(dec("0.9")), dec("0.525"));
    assert_eq!(borrow_rate(Dec::ONE), dec("1"));
    assert_eq!(supply_rate(dec("0.05"), dec("0.7424337969")), dec("0.0334095209"));
}

#[test]
fn a_reserve_values_collateral_and_says_what_it_can_still_lend() {
    let mainnet = reserves(Network::Mainnet);
    let hype = &reserve(&mainnet, 150).unwrap();
    assert_eq!(hype.value_of(dec("10")), Some(dec("835.09")));
    assert_eq!(hype.backing(dec("10")), Some(dec("542.8085")), "at its 0.65 loan to value");
    let usdc = reserve(&mainnet, 0).unwrap();
    assert_eq!(usdc.lendable(), dec("137147223.4528518021"));
    assert_eq!(usdc.backing(dec("100")), Some(Dec::ZERO), "USDC backs no borrow");
    let testnet = reserves(Network::Testnet);
    assert_eq!(reserve(&testnet, 1105).map(|hype| hype.ltv), Some(dec("0.65")), "HYPE is 1105 on testnet");
    assert!(reserve(&testnet, 150).is_none(), "and 150 is no reserve there");
}

/// atlas's `borrowable`: each held token's free amount at its oracle price
/// and loan to value, less what is owed, at most what USDC's reserve holds
/// and the user cap.
#[test]
fn what_can_still_be_borrowed() {
    let owes = |usdc: &str| -> UserState {
        serde_json::from_str(&format!(
            r#"{{"tokenToState":[[0,{{"borrow":{{"basis":"{usdc}","value":"{usdc}"}},"supply":{{"basis":"0.0","value":"0.0"}}}}]],"health":"healthy","healthFactor":null}}"#
        ))
        .unwrap()
    };
    let mainnet = reserves(Network::Mainnet);
    assert_eq!(borrowable(&[(150, dec("10"))], &mainnet, &owes("25")), dec("517.8085"));
    assert_eq!(
        borrowable(&[(150, dec("10")), (0, dec("1000"))], &mainnet, &owes("0")),
        dec("542.8085"),
        "USDC backs nothing"
    );
    assert_eq!(borrowable(&[(150, dec("10"))], &mainnet, &owes("600")), Dec::ZERO, "owing more than it backs");
    assert_eq!(borrowable(&[(150, dec("1000000000"))], &mainnet, &owes("0")), USER_BORROW_CAP, "the user cap");
    let testnet = reserves(Network::Testnet);
    assert_eq!(borrowable(&[(1105, dec("2"))], &testnet, &owes("0")), dec("90.51315"));
    assert_eq!(borrowable(&[(150, dec("2"))], &testnet, &owes("0")), Dec::ZERO, "testnet's 150 backs nothing");

    // atlas's account: everything it holds is held against, so nothing more.
    let atlas = atlas();
    let held: Vec<(u32, Dec)> = atlas
        .spot_clearinghouse_state
        .balances
        .iter()
        .map(|line| (line.token, line.total.checked_sub(line.hold).unwrap()))
        .collect();
    assert_eq!(borrowable(&held, &atlas.all_borrow_lend_reserve_states, &atlas.borrow_lend_user_state), Dec::ZERO);
}

/// atlas's words: `healthy` is safe, `liquidatable` is, and anything else
/// (the live `atRisk`) is near.
#[test]
fn health_reads_as_a_risk() {
    let state = atlas().borrow_lend_user_state;
    assert_eq!(state.health, "atRisk");
    assert_eq!(state.risk(), Risk::Near);
    assert_eq!(state.factor(), Some(dec("0.994555261")));
    assert_eq!(state.borrowed(0), dec("188.39764406"));
    assert_eq!(state.supplied(150), dec("3.32730523"));
    assert_eq!(state.position(150).map(|position| position.borrow.basis), Some(Dec::ZERO), "no basis written");
    assert_eq!(state.positions().count(), 2);
    let owed = state.position(0).unwrap().borrow;
    assert_eq!(owed.interest(), Some(dec("0.00064406")), "owed now less what was borrowed");
    for (word, risk) in [("healthy", Risk::Safe), ("liquidatable", Risk::Liquidatable), ("somethingNew", Risk::Near)] {
        let state: UserState =
            serde_json::from_str(&format!(r#"{{"tokenToState":[],"health":"{word}","healthFactor":null}}"#)).unwrap();
        assert_eq!(state.risk(), risk, "{word}");
    }
}
