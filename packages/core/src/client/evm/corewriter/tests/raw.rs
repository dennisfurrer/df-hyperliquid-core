//! The decoder against live `RawAction` logs off `0x3333...3333` on mainnet (windows captured
//! 2026-09-26 and 2026-10-08, and a send's receipt), each read by viem's
//! `decodeAbiParameters` as well.

use serde_json::Value;

use crate::client::evm::corewriter::SPOT_DEX;
use crate::client::evm::corewriter::support::{action, hex};
use crate::client::evm::fixtures;
use crate::client::evm::rpc::eth;
use crate::client::evm::u256::B256;

use super::*;

/// A log as a vector keeps it. Some were filtered on CoreWriter alone and
/// kept without their `address`, so it is put back.
fn log(value: &Value) -> Log {
    let mut value = value.clone();
    if value.get("address").is_none() {
        value["address"] = Value::String(CORE_WRITER.to_hex());
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn live_mainnet_logs_decode_and_round_trip() {
    let file = fixtures::cases();
    let logs = file["corewriter"]["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 8);
    for raw in logs {
        let read = RawAction::from_log(&log(raw)).unwrap();
        let action = read.action.clone().unwrap();
        assert_eq!(action.payload(), read.payload, "the encoder gives back the logged bytes");
    }
    // The first: USDC's system contract sending asset, as viem read it.
    let first = RawAction::from_log(&log(&logs[0])).unwrap();
    assert_eq!(first.user.to_hex(), "0x6b9e773128f453f5c2c60935ee2de2cbc5390a24");
    assert_eq!(
        first.action,
        Ok(Action::SendAsset {
            destination: fixtures::address("0xc0a6a8da035f8010bdc280e16f4bc2af415bb0c3"),
            sub_account: Address::ZERO,
            source_dex: SPOT_DEX,
            destination_dex: 0,
            token: 0,
            wei: 3_814_000_000,
        })
    );
    // The seventh: a staking deposit.
    assert_eq!(
        RawAction::from_log(&log(&logs[6])).unwrap().action,
        Ok(Action::StakingDeposit { wei: 1_000_249_648_235 })
    );
}

#[test]
fn a_live_window_of_logs_decodes_as_viem_reads_it() {
    let answer = fixtures::answer("eth_getLogs", 1);
    let logs = eth::logs(&raw_action_logs(
        crate::client::evm::rpc::BlockTag::Latest,
        crate::client::evm::rpc::BlockTag::Latest,
    ))
    .answer(&answer)
    .unwrap();
    let viem = fixtures::viem();
    let expected = viem["liveLogs"].as_array().unwrap();
    assert_eq!(logs.len(), expected.len());
    for (one, said) in logs.iter().zip(expected) {
        let read = RawAction::from_log(one).unwrap();
        assert_eq!(read.user.to_hex(), fixtures::text(&said["user"]));
        assert_eq!(read.version, 1);
        assert_eq!(read.action, Ok(action(said["id"].as_u64().unwrap(), said["values"].as_array().unwrap())));
    }
}

#[test]
fn a_live_sends_receipt_carries_its_action() {
    let receipt = eth::receipt(B256::ZERO).answer(&fixtures::answer("eth_getTransactionReceipt", 0)).unwrap().unwrap();
    let actions: Vec<RawAction> = receipt.logs.iter().filter_map(|one| RawAction::from_log(one).ok()).collect();
    assert_eq!(actions.len(), 1, "one RawAction among the USDC logs");
    let viem = fixtures::viem();
    assert_eq!(hex(&actions[0].payload), fixtures::text(&viem["rawAction"]["payload"]));
    assert!(matches!(actions[0].action, Ok(Action::SendAsset { .. })));
    let usdc_log = receipt.logs.iter().find(|one| one.address != CORE_WRITER).unwrap();
    assert_eq!(RawAction::from_log(usdc_log), Err(BadAction::NotRawAction), "another contract's log");
}
