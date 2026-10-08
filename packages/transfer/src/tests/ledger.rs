//! `ledger.json` is one live mainnet row of every type seen on 2026-10-06,
//! keyed by type, as the venue sent it.

use std::collections::BTreeMap;

use hl_core::Dec;

use super::*;

const LIVE: &str = include_str!("ledger.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn rows() -> BTreeMap<String, LedgerUpdate> {
    serde_json::from_str(LIVE).unwrap()
}

#[test]
fn every_live_row_reads_and_keeps_its_type() {
    let rows = rows();
    assert_eq!(rows.len(), 12);
    for (kind, row) in &rows {
        assert_eq!(row.delta.kind(), kind);
        assert!(row.hash.starts_with("0x") && row.hash.len() == 66, "{kind}");
        assert!(row.time > 1_700_000_000_000, "{kind}");
    }
}

#[test]
fn the_modelled_types_read_field_for_field() {
    let rows = rows();
    let delta = |kind: &str| rows[kind].delta.clone();
    let user = address("0x51172933b60847085e2a959e860e2ec9e240ac09");
    assert_eq!(
        delta("send"),
        Delta::Send {
            user,
            destination: user,
            source_dex: String::new(),
            destination_dex: "spot".to_owned(),
            token: "USDC".to_owned(),
            amount: dec("2550"),
            usdc_value: dec("2550"),
            fee: Dec::ZERO,
            native_token_fee: Some(Dec::ZERO),
            nonce: Some(1_764_137_111_508),
            fee_token: String::new(),
        }
    );
    assert_eq!(
        delta("internalTransfer"),
        Delta::InternalTransfer {
            usdc: dec("2462"),
            user,
            destination: address("0xac84a81cd786abde89136b77482866905106adb6"),
            fee: Dec::ONE,
        }
    );
    assert_eq!(
        delta("spotTransfer"),
        Delta::SpotTransfer {
            token: "KNTQ".to_owned(),
            amount: dec("10"),
            usdc_value: dec("1.55759"),
            user,
            destination: address("0x200000000000000000000000000000000000007c"),
            fee: Dec::ZERO,
            native_token_fee: Some(dec("0.00002")),
            nonce: Some(1_764_245_231_181),
            fee_token: String::new(),
        }
    );
    assert_eq!(delta("deposit"), Delta::Deposit { usdc: dec("2525.99") });
    assert_eq!(
        delta("withdraw"),
        Delta::Withdraw { usdc: dec("52912.218999"), nonce: 1_759_909_832_650_000, fee: Dec::ONE }
    );
    assert_eq!(delta("accountClassTransfer"), Delta::AccountClassTransfer { usdc: dec("49981.11"), to_perp: true });
    assert_eq!(
        delta("accountActivationGas"),
        Delta::AccountActivationGas { amount: Dec::ONE, token: "USDC".to_owned() }
    );
    let hlp = address("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303");
    assert_eq!(delta("vaultDeposit"), Delta::VaultDeposit { vault: hlp, usdc: dec("58499.16") });
    assert_eq!(
        delta("vaultWithdraw"),
        Delta::VaultWithdraw {
            vault: hlp,
            user: address("0x5f3df357144f748d7d53f30719a6e4040e2c7d04"),
            requested_usd: dec("24987.57"),
            commission: Dec::ZERO,
            closing_cost: Dec::ZERO,
            basis: dec("25000.721474"),
            net_withdrawn_usd: dec("24987.57"),
        }
    );
}

/// HYPE in and out of staking (`cDeposit`, `cWithdraw`), a deployer's
/// genesis share and a gas auction's bid.
#[test]
fn the_staking_genesis_and_auction_rows_read() {
    let rows = rows();
    assert_eq!(
        rows["cStakingTransfer"].delta,
        Delta::CStakingTransfer { token: "HYPE".to_owned(), amount: dec("2915.2795455"), is_deposit: true }
    );
    assert_eq!(rows["spotGenesis"].delta, Delta::SpotGenesis { token: "KNTQ".to_owned(), amount: dec("730000000") });
    assert_eq!(
        rows["deployGasAuction"].delta,
        Delta::DeployGasAuction { token: "HYPE".to_owned(), amount: dec("500") }
    );
    assert!(rows.values().all(|row| !matches!(row.delta, Delta::Other { .. })), "every live type is modelled");
}

/// A send's row carries the nonce it was signed with, so a read back finds
/// the very action; rows without one are never matched.
#[test]
fn a_send_is_found_in_the_ledger_by_its_nonce() {
    let rows: Vec<LedgerUpdate> = rows().into_values().collect();
    assert_eq!(find(&rows, 1_764_137_111_508).map(|row| row.delta.kind()), Some("send"));
    assert_eq!(find(&rows, 1_764_245_231_181).map(|row| row.delta.kind()), Some("spotTransfer"));
    assert_eq!(rows.iter().filter_map(|row| row.delta.nonce()).count(), 3, "send, spotTransfer, withdraw");
    assert_eq!(find(&rows, 1_759_909_832_650_000), None, "a withdrawal's nonce is not its action's");
    assert_eq!(find(&rows, 1), None);
}

#[test]
fn a_known_type_that_changed_shape_does_not_blank_the_list() {
    let text = r#"[
        {"time":1,"hash":"0x00","delta":{"type":"deposit","usdc":"5"}},
        {"time":2,"hash":"0x01","delta":{"type":"deposit","amount":"5"}},
        {"time":3,"hash":"0x02","delta":{"type":"somethingNew","x":[1,2]}},
        {"time":4,"hash":"0x03","delta":{"noType":true}}
    ]"#;
    let rows: Vec<LedgerUpdate> = serde_json::from_str(text).unwrap();
    assert_eq!(rows[0].delta, Delta::Deposit { usdc: dec("5") });
    let kept: Vec<(&str, String)> = rows[1..]
        .iter()
        .map(|row| match &row.delta {
            Delta::Other { kind, fields } => (kind.as_str(), fields.compact()),
            other => panic!("{other:?} read"),
        })
        .collect();
    assert_eq!(
        kept,
        [
            ("deposit", r#"{"type":"deposit","amount":"5"}"#.to_owned()),
            ("somethingNew", r#"{"type":"somethingNew","x":[1,2]}"#.to_owned()),
            ("", r#"{"noType":true}"#.to_owned()),
        ],
        "kept as written, every key and digit"
    );
}

/// Every recorded send carries its USDC value, fee and fee token: one without them is kept as a
/// row of its type unread, never read as a send of no value.
#[test]
fn a_send_missing_what_every_recording_carries_is_not_read_as_zero() {
    let live: serde_json::Value = serde_json::from_str(LIVE).unwrap();
    let mut send = live["send"].clone();
    send["delta"].as_object_mut().unwrap().remove("usdcValue");
    let row: LedgerUpdate = serde_json::from_value(send).unwrap();
    assert_eq!(row.delta.kind(), "send");
    assert!(!matches!(row.delta, Delta::Send { .. }), "unread, not a send worth nothing");
}

#[test]
fn the_request_carries_an_end_only_when_given() {
    let user = address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414");
    assert_eq!(
        ledger_request(user, 1_790_000_000_000, None),
        r#"{"type":"userNonFundingLedgerUpdates","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","startTime":1790000000000}"#
    );
    assert!(ledger_request(user, 1, Some(2)).ends_with(r#""startTime":1,"endTime":2}"#));
}
