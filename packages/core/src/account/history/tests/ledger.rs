//! Ledger rows as the venue writes them. `transfer/ledger.json` is one live mainnet row of every
//! type seen on 2026-10-06, keyed by type; `transfer/ledger_more.json` one each of `rewardsClaim` (a
//! builder's claim), `subAccountTransfer` and `vaultCreate` (its creation fee charged), read on
//! 2026-10-08. The read runs in the mocked journeys.

use std::collections::BTreeMap;

use crate::utils::Dec;

use super::*;

const LIVE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/transfer/ledger.json"));
const MORE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/transfer/ledger_more.json"));

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
fn every_live_row_type_reads_field_for_field() {
    let mut rows = rows();
    rows.extend(serde_json::from_str::<BTreeMap<String, LedgerUpdate>>(MORE).unwrap());
    let user = address("0x51172933b60847085e2a959e860e2ec9e240ac09");
    let hlp = address("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303");
    let expected = [
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
        },
        Delta::InternalTransfer {
            usdc: dec("2462"),
            user,
            destination: address("0xac84a81cd786abde89136b77482866905106adb6"),
            fee: Dec::ONE,
        },
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
        },
        Delta::Deposit { usdc: dec("2525.99") },
        Delta::Withdraw { usdc: dec("52912.218999"), nonce: 1_759_909_832_650_000, fee: Dec::ONE },
        Delta::AccountClassTransfer { usdc: dec("49981.11"), to_perp: true },
        Delta::AccountActivationGas { amount: Dec::ONE, token: "USDC".to_owned() },
        Delta::VaultDeposit { vault: hlp, usdc: dec("58499.16") },
        Delta::VaultWithdraw {
            vault: hlp,
            user: address("0x5f3df357144f748d7d53f30719a6e4040e2c7d04"),
            requested_usd: dec("24987.57"),
            commission: Dec::ZERO,
            closing_cost: Dec::ZERO,
            basis: dec("25000.721474"),
            net_withdrawn_usd: dec("24987.57"),
        },
        Delta::CStakingTransfer { token: "HYPE".to_owned(), amount: dec("2915.2795455"), is_deposit: true },
        Delta::SpotGenesis { token: "KNTQ".to_owned(), amount: dec("730000000") },
        Delta::DeployGasAuction { token: "HYPE".to_owned(), amount: dec("500") },
        Delta::RewardsClaim { amount: dec("37.41544941"), token: "USDC".to_owned() },
        Delta::SubAccountTransfer {
            usdc: dec("50"),
            user: address("0x7789450871fb1315fa982ccb8039cb34e8f2f60d"),
            destination: address("0xbe72e881983eaf61de3b63c56c488f0053266f04"),
        },
        Delta::VaultCreate {
            vault: address("0x9b5b1931fa5838a891973f9877c7648c93ee80cc"),
            usdc: dec("150"),
            fee: dec("100"),
        },
    ];
    assert_eq!(rows.len(), expected.len(), "one row of every type modelled");
    for delta in expected {
        let row = &rows[delta.kind()];
        assert_eq!(row.delta, delta);
        assert!(row.hash.starts_with("0x") && row.hash.len() == 66, "{}", row.kind());
        assert!(row.time > 1_700_000_000_000, "{}", row.kind());
    }
}

/// A send's row carries the nonce it was signed with, so a read back finds the very action; rows
/// without one are never matched.
#[test]
fn a_send_is_found_in_the_ledger_by_its_nonce() {
    let rows: Vec<LedgerUpdate> = rows().into_values().collect();
    assert_eq!(find(&rows, 1_764_137_111_508).map(LedgerUpdate::kind), Some("send"));
    assert_eq!(find(&rows, 1_764_245_231_181).map(LedgerUpdate::kind), Some("spotTransfer"));
    assert_eq!(rows.iter().filter_map(|row| row.delta.nonce()).count(), 3, "send, spotTransfer, withdraw");
    assert_eq!(find(&rows, 1_759_909_832_650_000), None, "a withdrawal's nonce is not its action's");
    assert_eq!(find(&rows, 1), None);
}

/// A type this build does not read, or a known one that no longer reads as modelled, is kept as
/// written, so one changed row never blanks the history; a send without the USDC value every
/// recorded send carries is never read as a send of no value.
#[test]
fn a_row_that_does_not_read_as_modelled_is_kept_as_written() {
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
            Delta::Other { fields, .. } => (row.kind(), fields.compact()),
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
    let mut live: serde_json::Value = serde_json::from_str(LIVE).unwrap();
    let mut send = live["send"].take();
    send["delta"].as_object_mut().unwrap().remove("usdcValue");
    let row: LedgerUpdate = serde_json::from_value(send).unwrap();
    assert_eq!(row.kind(), "send");
    assert!(matches!(row.delta, Delta::Other { .. }), "unread, not a send worth nothing");
}
