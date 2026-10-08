//! hip4-tui's `net/src/tests/fees.rs` on what a deployer's ledger explains, and one live mainnet
//! row of every type `userNonFundingLedgerUpdates` sent on 2026-10-06 (hl-transfer's recording),
//! each read for what it moved by hip4-tui's rules, worked through by hand.

use std::collections::BTreeMap;

use hl_account::Referral;
use hl_core::{Address, Dec};

use super::*;
use crate::{Basis, builders::Rewards};

const OUT: &str = "0x0c46eb73fae2816f219fcf11f50d6d3c59b5819e";
const EVERY_KIND: &str = include_str!("../../fixtures/ledger_rows_mainnet.json");
const REFERRAL: &str = include_str!("../../fixtures/referral_builder.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn rows(json: &str) -> Vec<LedgerRow> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn a_send_counts_for_the_end_that_is_this_address() {
    let ledger = rows(&format!(
        r#"[
          {{"time": 1, "hash": "0x00", "delta": {{"type": "send",
            "user": "0x1ec3d195795e92b8df60a428b2e6d24af59f050c",
            "destination": "{OUT}", "sourceDex": "spot",
            "destinationDex": "spot", "token": "USDC",
            "amount": "11.66291244", "usdcValue": "11.662912",
            "fee": "0.0", "nativeTokenFee": "0.0", "feeToken": ""}}}},
          {{"time": 2, "hash": "0x01", "delta": {{"type": "send",
            "user": "{OUT}",
            "destination": "0xfa288d234b58b109f4efd1dd5573d9bd7f948a5b",
            "sourceDex": "spot", "destinationDex": "spot",
            "token": "USDC", "amount": "3.0", "usdcValue": "3.0",
            "fee": "1.0", "nativeTokenFee": "0.0", "feeToken": "USDC"}}}}
        ]"#
    ));
    assert_eq!(explained(&ledger, address(OUT)), (dec("7.66291244"), true));
}

#[test]
fn a_row_this_build_cannot_read_makes_the_total_unknown() {
    let ledger = rows(r#"[{"time": 1, "hash": "0x00", "delta": {"type": "somethingNewThisWeek", "usdc": "100.0"}}]"#);
    let (moved, exact) = explained(&ledger, address(OUT));
    assert_eq!((moved, exact), (Dec::ZERO, false));
    let earned = Earned::of(Dec::from_int(500), &ledger, address(OUT));
    assert_eq!(earned.fees(), None, "a hole in the ledger is not a total");
}

#[test]
fn fees_are_what_the_balance_holds_that_no_row_explains() {
    let earned = Earned { held: dec("11310.86141184"), moved: dec("-64.33708756"), exact: true, ..Earned::default() };
    let fees = earned.fees().unwrap();
    assert_eq!(fees.amount, dec("11375.1984994"));
    assert_eq!(fees.basis, Basis::Ledger, "an estimate, and said to be one");
}

#[test]
fn every_kind_of_row_the_venue_sent_moves_what_its_kind_says() {
    let live: BTreeMap<String, LedgerRow> = serde_json::from_str(EVERY_KIND).unwrap();
    let me = address("0x51172933b60847085e2a959e860e2ec9e240ac09");
    let moved: BTreeMap<&str, Option<Dec>> = live.iter().map(|(kind, row)| (kind.as_str(), row.moved(me))).collect();
    let expected: BTreeMap<&str, Option<Dec>> = [
        ("deposit", Some(dec("2525.99"))),
        ("send", Some(dec("2550"))),
        ("spotGenesis", Some(Dec::ZERO)),
        ("spotTransfer", Some(Dec::ZERO)),
        ("internalTransfer", Some(dec("-2462"))),
        ("deployGasAuction", None),
        ("accountActivationGas", None),
        ("accountClassTransfer", Some(dec("-49981.11"))),
        ("withdraw", Some(dec("-52912.218999"))),
        ("vaultDeposit", Some(dec("-58499.16"))),
        ("vaultWithdraw", None),
        ("cStakingTransfer", Some(Dec::ZERO)),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        moved, expected,
        "a send to itself arrives; another token's moves no USDC; a fee counts only where USDC is its token; \
         the gas auction, activation gas and a vault withdrawal are not read"
    );
}

#[test]
fn a_spot_class_transfer_comes_back_and_a_charged_fee_goes() {
    let ledger = rows(&format!(
        r#"[{{"time": 1, "hash": "0x00", "delta": {{"type": "accountClassTransfer", "usdc": "40.5", "toPerp": false}}}},
            {{"time": 2, "hash": "0x01", "delta": {{"type": "subAccountTransfer", "usdc": "10.0",
              "user": "0x00000000000000000000000000000000000000aa", "destination": "{OUT}"}}}},
            {{"time": 3, "hash": "0x02", "delta": {{"type": "rewardsClaim", "usdc": "2.25"}}}},
            {{"time": 4, "hash": "0x03", "delta": {{"type": "vaultCreate", "usdc": "100.0", "fee": "1.0",
              "feeToken": "USDC", "user": "{OUT}"}}}}]"#
    ));
    assert_eq!(explained(&ledger, address(OUT)), (dec("-48.25"), true), "40.5 + 10 + 2.25 - 100 - 1");
}

#[test]
fn what_a_venue_was_paid_names_its_builder_code_where_it_has_one() {
    let referral: Referral = serde_json::from_str(REFERRAL).unwrap();
    let own = Rewards { builder: dec("1.5"), ..Rewards::default() };
    let earned = Earned { rewards: Some(own.clone()), ..Earned::default() };
    assert_eq!(earned.paid(), Some(&own));
    let coded = Earned { coded: Some(Rewards::of(&referral)), ..earned };
    assert_eq!(
        coded.paid().map(|paid| paid.builder),
        Some(dec("82.23638757")),
        "the code's address, not the deployer's"
    );
    assert_eq!(Earned::default().paid(), None);
}
