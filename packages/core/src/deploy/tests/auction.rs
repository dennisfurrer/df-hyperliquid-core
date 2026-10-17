//! `perpDeployAuctionStatus` as mainnet (ended) and testnet (running) answered it.

use super::*;
use crate::utils::dec::Dec;

const ENDED: &str = r#"{"startTimeSeconds":1790398800,"durationSeconds":111600,"startGas":"1109.77867352","currentGas":null,"endGas":"541.69363865"}"#;
const RUNNING: &str = r#"{"startTimeSeconds":1790398800,"durationSeconds":111600,"startGas":"1000.0","currentGas":"527.03134408","endGas":null}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn an_auction_reads_its_round_in_seconds_and_its_gas_in_hype() {
    let ended: GasAuction = serde_json::from_str(ENDED).unwrap();
    assert_eq!((ended.start_time_seconds, ended.duration_seconds), (1_790_398_800, 31 * 3600));
    assert_eq!(
        (ended.start_gas, ended.current_gas, ended.end_gas),
        (dec("1109.77867352"), None, Some(dec("541.69363865")))
    );
    let running: GasAuction = serde_json::from_str(RUNNING).unwrap();
    assert_eq!(
        (running.start_gas, running.current_gas, running.end_gas),
        (dec("1000"), Some(dec("527.03134408")), None)
    );
}

#[test]
fn an_auction_runs_from_its_start_until_its_end() {
    let running: GasAuction = serde_json::from_str(RUNNING).unwrap();
    assert_eq!(running.ends_at(), 1_790_398_800 + 111_600);
    assert!(running.is_running(running.start_time_seconds));
    assert!(running.is_running(running.ends_at() - 1));
    assert!(!running.is_running(running.ends_at()), "over at its end");
    assert!(!running.is_running(running.start_time_seconds - 1), "not before its start");
}
