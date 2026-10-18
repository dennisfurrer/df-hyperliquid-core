//! `perpDeployAuctionStatus` as mainnet (ended) and testnet (running) answered it: a round in
//! seconds, its gas in HYPE, and when it runs.

use super::*;
use crate::utils::dec::Dec;

const ENDED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_deploy_auction_mainnet.json"));
const RUNNING: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_deploy_auction_testnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// A round runs from its start until, not including, its end; once over it has no current gas and
/// says what the slot sold at.
#[test]
fn an_auction_reads_its_round_in_seconds_and_runs_from_its_start_until_its_end() {
    let ended: GasAuction = serde_json::from_str(ENDED).unwrap();
    assert_eq!((ended.start_time_seconds, ended.duration_seconds), (1_790_398_800, 31 * 3600));
    assert_eq!(
        (ended.start_gas, ended.current_gas, ended.end_gas),
        (dec("1109.77867352"), None, Some(dec("541.69363865")))
    );
    let running: GasAuction = serde_json::from_str(RUNNING).unwrap();
    assert_eq!((running.current_gas, running.end_gas), (Some(dec("527.03134408")), None));
    assert_eq!(running.ends_at(), running.start_time_seconds + 111_600);
    assert!(running.is_running(running.start_time_seconds) && running.is_running(running.ends_at() - 1));
    assert!(!running.is_running(running.ends_at()), "over at its end");
    assert!(!running.is_running(running.start_time_seconds - 1), "not before its start");
}
