use crate::client::USDC_INDEX;

use super::*;
use crate::account::fixtures::{
    BUILDER_SPOT, PERP_UNIFIED, PERP_UNIFIED_XYZ, SPOT_PORTFOLIO_MARGIN, SPOT_STANDARD, SPOT_UNIFIED, UNIFIED, address,
    dec, read,
};

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

#[test]
fn a_clearinghouse_state_reads_every_position_and_its_margin() {
    let perp: PerpState = read(PERP_UNIFIED);
    let summary = &perp.margin_summary;
    assert_eq!(
        (summary.account_value, summary.total_ntl_pos, summary.total_margin_used),
        (dec("549463.356168"), dec("2401316.1375199999"), dec("480263.227504"))
    );
    assert_eq!(perp.cross_maintenance_margin_used, dec("44258.806876"));
    assert_eq!(perp.time, Some(1_791_480_973_789));
    let coins: Vec<&str> = perp.positions().map(|held| held.coin.as_str()).collect();
    assert_eq!(coins, ["BTC", "ETH", "NEAR"]);
    let btc = &perp.asset_positions[0].position;
    assert_eq!(btc.szi, dec("-22"));
    assert!(btc.is_short());
    assert_eq!(btc.entry_px, Some(dec("81134.2")));
    assert_eq!(btc.unrealized_pnl, dec("5812.516083"));
    assert_eq!(btc.liquidation_px, Some(dec("136313.4115825814")));
    assert_eq!((btc.leverage.mode(), btc.leverage.value()), (MarginMode::Cross, 5));
    assert!(perp.is_used());
    let xyz: PerpState = read(PERP_UNIFIED_XYZ);
    assert!(!xyz.is_used(), "a dex never traded on");
    assert_eq!(xyz.positions().count(), 0);
}

/// Every recorded clearinghouse, read or sent, carries its cross summary, its cross maintenance
/// and each position's value and leverage whole: one without them is refused, not read as zero.
#[test]
fn a_clearinghouse_missing_what_every_recording_carries_is_refused() {
    let whole: serde_json::Value = serde_json::from_str(PERP_UNIFIED).unwrap();
    assert!(serde_json::from_value::<PerpState>(whole.clone()).is_ok());
    for gone in ["crossMarginSummary", "crossMaintenanceMarginUsed"] {
        let mut without = whole.clone();
        without.as_object_mut().unwrap().remove(gone);
        assert!(serde_json::from_value::<PerpState>(without).is_err(), "without {gone}");
    }
    for gone in ["positionValue", "maxLeverage"] {
        let mut without = whole.clone();
        without["assetPositions"][0]["position"].as_object_mut().unwrap().remove(gone);
        assert!(serde_json::from_value::<PerpState>(without).is_err(), "without {gone}");
    }
    let mut isolated = whole;
    isolated["assetPositions"][0]["position"]["leverage"] = serde_json::json!({"type": "isolated", "value": 5});
    assert!(serde_json::from_value::<PerpState>(isolated).is_err(), "an isolated leverage without its rawUsd");
}

/// A flat leg, made by hand in the venue's whole shape. A figure left out was once read as
/// nothing; every recorded state carries them all, so now it is refused.
#[test]
fn a_flat_leg_is_not_a_position() {
    let flat = r#"{"marginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"1.0","assetPositions":[{"type":"oneWay","position":{"coin":"BTC","szi":"0.0","leverage":{"type":"cross","value":5},"entryPx":null,"positionValue":"0.0","unrealizedPnl":"0.0","marginUsed":"0.0","maxLeverage":40}}]}"#;
    let perp: PerpState = read(flat);
    assert_eq!(perp.positions().count(), 0);
    assert!(perp.is_used(), "it holds a dollar");
    let left_out = flat.replace(r#""totalNtlPos":"0.0","#, "");
    assert!(serde_json::from_str::<PerpState>(&left_out).is_err(), "a figure left out is refused, not read as nothing");
}

#[test]
fn one_dex_is_asked_for_by_name_and_the_main_dex_by_none() {
    let user = address(UNIFIED);
    assert_eq!(
        perp_state_on_request(user, "xyz"),
        format!(r#"{{"type":"clearinghouseState","user":"{UNIFIED}","dex":"xyz"}}"#)
    );
    assert_eq!(perp_state_on_request(user, ""), perp_state_request(user));
}

#[test]
fn dex_abstraction_is_a_mode_of_its_own_and_keeps_ledgers_apart() {
    let mode: Abstraction = read("\"dexAbstraction\"");
    assert_eq!(mode, Abstraction::DexAbstraction);
    assert!(!mode.is_pooled());
    assert!(mode.is_standard());
    assert_eq!(mode.wire(), "dexAbstraction");
    for standard in [Abstraction::Default, Abstraction::Disabled, Abstraction::Unknown("new".to_owned())] {
        assert!(standard.is_standard(), "{standard}");
    }
    for pooled in [Abstraction::UnifiedAccount, Abstraction::PortfolioMargin] {
        assert!(!pooled.is_standard(), "{pooled}");
    }
}
