//! Live answers as the venue wrote them: one account's spot on mainnet, 2026-10-06, and the
//! recorded fixtures.

use crate::client::USDC_INDEX;

use super::*;
use crate::account::fixtures::{BUILDER_SPOT, SPOT_PORTFOLIO_MARGIN, SPOT_STANDARD, SPOT_UNIFIED, dec, read};

/// Live answers, mainnet, 2026-10-06.
const SPOT: &str = r#"{"balances":[{"coin":"USDC","token":0,"total":"8155.50418728","hold":"0.0","entryNtl":"0.0"},{"coin":"PURR","token":1,"total":"62.97481","hold":"0.0","entryNtl":"14.6492003"},{"coin":"HYPE","token":150,"total":"0.00000004","hold":"0.0","entryNtl":"0.00000359"}]}"#;
/// A unified account holding HIP-4 outcome rows, which have no `token`, and
/// the pooled extra `tokenToAvailableAfterMaintenance` (live, 2026-10-06).
const SPOT_WITH_OUTCOMES: &str = r#"{"balances":[{"coin":"USDC","token":0,"total":"11.515057","hold":"11.515057","entryNtl":"0.0"},{"coin":"USDE","token":235,"total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92260","total":"0.0","hold":"0.0","entryNtl":"0.0"},{"coin":"+92261","total":"0.0","hold":"0.0","entryNtl":"0.0"}],"tokenToAvailableAfterMaintenance":[[0,"8.554471"]]}"#;

#[test]
fn spot_state_reads_the_live_answer_and_finds_usdc_by_index() {
    let state: SpotState = serde_json::from_str(SPOT).unwrap();
    assert_eq!(state.balances.len(), 3);
    let usdc = state.usdc().unwrap();
    assert_eq!(
        (usdc.coin.as_str(), usdc.token, usdc.total, usdc.hold),
        ("USDC", Some(0), dec("8155.50418728"), Dec::ZERO)
    );
    assert_eq!(state.balance(150).unwrap().total, dec("0.00000004"));
    let empty: SpotState = serde_json::from_str(r#"{"balances":[]}"#).unwrap();
    assert_eq!(empty.usdc(), None);
}

/// Portfolio margin, live 2026-10-06: a negative `hold` and a `spotHold`
/// holding the whole (tiny) USDC balance. `total - hold` would read $269.38
/// as free; nothing is.
#[test]
fn a_negative_hold_never_makes_money_available() {
    let state: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"0.55817531","hold":"-268.81821068","entryNtl":"0.0","spotHold":"0.55817531","ltv":"0.0","borrowed":"89.3303277"},{"coin":"HYPE","token":150,"total":"6.02901107","hold":"1.50143146","entryNtl":"0.0","ltv":"0.65","supplied":"6.02901107"}]}"#,
    )
    .unwrap();
    let usdc = state.usdc().unwrap();
    assert_eq!(usdc.available(), Some(Dec::ZERO));
    assert_eq!(usdc.borrowed, Some(dec("89.3303277")));
    let hype = state.balance(150).unwrap();
    assert_eq!(hype.available(), Some(dec("4.52757961")));
    assert_eq!(hype.supplied, Some(dec("6.02901107")));

    let free: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"50.0","hold":"-10.0","entryNtl":"0.0"}]}"#,
    )
    .unwrap();
    assert_eq!(free.usdc().unwrap().available(), Some(dec("50")), "never above total");
    let owed: SpotState = serde_json::from_str(
        r#"{"balances":[{"coin":"USDC","token":0,"total":"-18.9","hold":"-18.9","entryNtl":"0.0"}]}"#,
    )
    .unwrap();
    assert_eq!(owed.usdc().unwrap().available(), Some(Dec::ZERO), "never below zero");
}

#[test]
fn outcome_rows_without_a_token_do_not_break_the_read() {
    let state: SpotState = serde_json::from_str(SPOT_WITH_OUTCOMES).unwrap();
    assert_eq!(state.balances.len(), 4);
    assert_eq!(state.balances[2].token, None);
    let usdc = state.usdc().unwrap();
    assert_eq!((usdc.total, usdc.available()), (dec("11.515057"), Some(Dec::ZERO)));
}

/// What the venue says is held is the authoritative answer, largest first, an empty row left out.
#[test]
fn what_is_held_is_every_row_with_something_in_it_largest_first() {
    let spot: SpotState = read(SPOT_STANDARD);
    let held: Vec<&str> = spot.held().map(|row| row.coin.as_str()).collect();
    assert_eq!(held, ["MAX", "MUNCH", "USDC", "PURR", "HYPE", "UBTC"]);
    let builder: SpotState = read(BUILDER_SPOT);
    assert_eq!(builder.held().map(|row| row.coin.as_str()).collect::<Vec<_>>(), ["USDC"], "the empty rows are none");
}

#[test]
fn a_pooled_spot_state_says_what_each_token_has_free_after_maintenance() {
    let spot: SpotState = read(SPOT_UNIFIED);
    assert_eq!(spot.portfolio_margin_enabled, None, "said only where it is on");
    assert_eq!(spot.available_after_maintenance(USDC_INDEX), Some(dec("1235001.99300258")));
    assert_eq!(spot.available_after_maintenance(150), None, "the venue named USDC alone");
    // 1279260.79987858 less its hold of 543038.7473040001.
    assert_eq!(spot.free(USDC_INDEX), dec("736222.0525745799"));
    assert_eq!(spot.free(150), dec("0.00722909"));
    assert_eq!(spot.free(9_999), Dec::ZERO, "a token never held is nothing free");
}

/// Every recorded spot balance carries its entry notional: one without it is refused, not read
/// as nothing paid.
#[test]
fn a_spot_balance_without_its_entry_notional_is_refused() {
    let whole: serde_json::Value = serde_json::from_str(SPOT_STANDARD).unwrap();
    let mut without = whole.clone();
    without["balances"][0].as_object_mut().unwrap().remove("entryNtl");
    assert!(serde_json::from_value::<SpotState>(whole).is_ok());
    assert!(serde_json::from_value::<SpotState>(without).is_err());
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

#[test]
fn outcome_rows_are_never_tokens() {
    let spot: SpotState = read(BUILDER_SPOT);
    let coins: Vec<(&str, bool)> = spot.balances.iter().map(|row| (row.coin.as_str(), row.is_outcome())).collect();
    assert_eq!(coins, [("USDC", false), ("HYPE", false), ("o458", true), ("+96380", true), ("+96381", true)]);
    assert!(spot.balances.iter().filter(|row| row.is_outcome()).all(|row| row.token.is_none()));
}
