//! `spotClearinghouseState` as the venue writes it, and what is free of it: live answers on
//! mainnet, 2026-10-02 to 10-08, and rows made by hand where a rule is the point. The read runs in
//! the mocked journeys.

use crate::client::USDC_INDEX;

use super::*;
use crate::account::fixtures::{BUILDER_SPOT, SPOT_PORTFOLIO_MARGIN, SPOT_STANDARD, SPOT_UNIFIED, dec, read};

/// A unified account holding HIP-4 outcome rows, which have no `token`, live, 2026-10-06.
const SPOT_WITH_OUTCOMES: &str = r#"{"balances":[{"coin":"USDC","token":0,"total":"11.515057","hold":"11.515057","entryNtl":"0.0"},{"coin":"USDE","token":235,"total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92260","total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92261","total":"0.0","hold":"0.0","entryNtl":"0.0"}],"tokenToAvailableAfterMaintenance":[[0,"8.554471"]]}"#;

/// What the venue says is held is the authoritative answer: every row with something in it,
/// largest first. Every recorded row carries its hold and entry notional: one without them is
/// refused, not read as nothing held or paid.
#[test]
fn a_spot_state_reads_every_row_finds_usdc_by_index_and_lists_what_is_held() {
    let unified: SpotState = read(SPOT_UNIFIED);
    assert_eq!(unified.portfolio_margin_enabled, None, "said only where it is on");
    let usdc = unified.usdc().unwrap();
    assert_eq!((usdc.coin.as_str(), usdc.token, usdc.total), ("USDC", Some(USDC_INDEX), dec("1279260.79987858")));
    assert_eq!(unified.available_after_maintenance(USDC_INDEX), Some(dec("1235001.99300258")));
    assert_eq!(unified.available_after_maintenance(150), None, "the venue named USDC alone");
    assert_eq!(unified.free(USDC_INDEX), dec("736222.0525745799"), "less its hold of 543038.7473040001");
    assert_eq!(
        (unified.free(150), unified.free(9_999)),
        (dec("0.00722909"), Dec::ZERO),
        "a token never held is nothing"
    );

    let standard: SpotState = read(SPOT_STANDARD);
    let held: Vec<&str> = standard.held().map(|row| row.coin.as_str()).collect();
    assert_eq!(held, ["MAX", "MUNCH", "USDC", "PURR", "HYPE", "UBTC"]);
    let builder: SpotState = read(BUILDER_SPOT);
    assert_eq!(builder.held().map(|row| row.coin.as_str()).collect::<Vec<_>>(), ["USDC"], "the empty rows are none");
    let outcomes: SpotState = read(SPOT_WITH_OUTCOMES);
    assert_eq!((outcomes.balances[2].token, outcomes.balance(235).map(|row| row.total)), (None, Some(Dec::ZERO)));
    assert_eq!(read::<SpotState>(r#"{"balances":[]}"#).usdc(), None, "an account that never held USDC");

    let whole: serde_json::Value = serde_json::from_str(SPOT_STANDARD).unwrap();
    for gone in ["hold", "entryNtl"] {
        let mut without = whole.clone();
        without["balances"][0].as_object_mut().unwrap().remove(gone);
        assert!(serde_json::from_value::<SpotState>(without).is_err(), "without {gone}");
    }
}

/// Portfolio margin's hold can be negative (live, 2026-10-06: `total - hold` would read $269.38 of
/// USDC as free, and nothing is). Free is the total less the larger positive hold, within nothing
/// and the total.
#[test]
fn what_is_free_is_the_total_less_the_larger_positive_hold_never_below_nothing_nor_above_the_total() {
    let row = |total: &str, hold: &str, spot_hold: Option<&str>| SpotBalance {
        total: dec(total),
        hold: dec(hold),
        spot_hold: spot_hold.map(dec),
        ..SpotBalance::default()
    };
    for (balance, free) in [
        (row("0.55817531", "-268.81821068", Some("0.55817531")), "0"),
        (row("6.02901107", "1.50143146", None), "4.52757961"),
        (row("50.0", "-10.0", None), "50"),
        (row("-18.9", "-18.9", None), "0"),
        (row("11.515057", "11.515057", None), "0"),
        (row("100", "20", Some("30")), "70"),
    ] {
        assert_eq!(balance.available(), Some(dec(free)), "{balance:?}");
    }
}

#[test]
fn portfolio_margin_reads_its_ratio_and_how_near_liquidation_it_stands() {
    let spot: SpotState = read(SPOT_PORTFOLIO_MARGIN);
    assert_eq!(spot.portfolio_margin_enabled, Some(true));
    assert_eq!(spot.portfolio_margin_ratio, Some(dec("0.1460346663")));
    assert_eq!(spot.risk(), Risk::Safe);
    assert_eq!(spot.balance(150).and_then(|hype| hype.supplied), Some(dec("3.32730523")));
    assert_eq!(spot.usdc().and_then(|usdc| usdc.borrowed), Some(dec("188.39764406")));
    // A warning from 0.85, liquidatable past 0.95.
    for (ratio, risk) in [
        (None, Risk::Safe),
        (Some("0.8499"), Risk::Safe),
        (Some("0.85"), Risk::Near),
        (Some("0.95"), Risk::Near),
        (Some("0.9500001"), Risk::Liquidatable),
    ] {
        assert_eq!(Risk::of(ratio.map(dec)), risk, "{ratio:?}");
    }
}

/// A HIP-4 row is a side (`+92260`, `#92260`) or a question (`o458`): never collateral, never a
/// token, so `balance` passes it by.
#[test]
fn outcome_rows_are_never_tokens() {
    for (coin, outcome) in [
        ("USDC", false),
        ("HYPE", false),
        ("o458", true),
        ("+96380", true),
        ("#96380", true),
        ("o", false),
        ("oX1", false),
    ] {
        let row = SpotBalance { coin: coin.to_owned(), ..SpotBalance::default() };
        assert_eq!(row.is_outcome(), outcome, "{coin}");
    }
    let spot = SpotState {
        balances: vec![SpotBalance {
            coin: "+0".to_owned(),
            token: Some(0),
            total: Dec::ONE,
            ..SpotBalance::default()
        }],
        ..SpotState::default()
    };
    assert_eq!(spot.usdc(), None, "an outcome row is no USDC, whatever index it carries");
}
