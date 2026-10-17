//! Live answers as the venue wrote them: one account's main dex on mainnet, 2026-10-06, and the
//! recorded fixtures.

use crate::markets::Dex;

use super::*;
use crate::account::fixtures::{PERP_UNIFIED, PERP_UNIFIED_XYZ, UNIFIED, address, dec, read};

/// Live answers, mainnet, 2026-10-06.
const PERP: &str = r#"{"marginSummary":{"accountValue":"41085992.9213469997","totalNtlPos":"0.0","totalRawUsd":"41085992.9213469997","totalMarginUsed":"477.867397"},"crossMarginSummary":{"accountValue":"41085515.0539499968","totalNtlPos":"0.0","totalRawUsd":"41085515.0539499968","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"41085515.0539499968","assetPositions":[],"time":1791304588904}"#;
const FRESH_PERP: &str = r#"{"marginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"0.0","totalNtlPos":"0.0","totalRawUsd":"0.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"0.0","assetPositions":[],"time":1791305116226}"#;

#[test]
fn perp_state_reads_what_it_needs_and_ignores_the_rest() {
    let state: ClearinghouseState = serde_json::from_str(PERP).unwrap();
    assert_eq!(state.withdrawable, dec("41085515.0539499968"));
    assert_eq!(state.margin_summary.account_value, dec("41085992.9213469997"));
    let fresh: ClearinghouseState = serde_json::from_str(FRESH_PERP).unwrap();
    assert_eq!(fresh.withdrawable, Dec::ZERO);
}

#[test]
fn a_clearinghouse_state_reads_every_position_and_its_margin() {
    let perp: ClearinghouseState = read(PERP_UNIFIED);
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
    assert!(perp.holds_anything());
    let xyz: ClearinghouseState = read(PERP_UNIFIED_XYZ);
    assert!(!xyz.holds_anything(), "a dex never traded on");
    assert_eq!(xyz.positions().count(), 0);
}

/// Every recorded clearinghouse, read or sent, carries its cross summary, its cross maintenance
/// and each position's value and leverage whole: one without them is refused, not read as zero.
#[test]
fn a_clearinghouse_missing_what_every_recording_carries_is_refused() {
    let whole: serde_json::Value = serde_json::from_str(PERP_UNIFIED).unwrap();
    assert!(serde_json::from_value::<ClearinghouseState>(whole.clone()).is_ok());
    for gone in ["crossMarginSummary", "crossMaintenanceMarginUsed"] {
        let mut without = whole.clone();
        without.as_object_mut().unwrap().remove(gone);
        assert!(serde_json::from_value::<ClearinghouseState>(without).is_err(), "without {gone}");
    }
    for gone in ["positionValue", "maxLeverage"] {
        let mut without = whole.clone();
        without["assetPositions"][0]["position"].as_object_mut().unwrap().remove(gone);
        assert!(serde_json::from_value::<ClearinghouseState>(without).is_err(), "without {gone}");
    }
    let mut isolated = whole;
    isolated["assetPositions"][0]["position"]["leverage"] = serde_json::json!({"type": "isolated", "value": 5});
    assert!(serde_json::from_value::<ClearinghouseState>(isolated).is_err(), "an isolated leverage without its rawUsd");
}

/// A flat leg, made by hand in the venue's whole shape. A figure left out was once read as
/// nothing; every recorded state carries them all, so now it is refused.
#[test]
fn a_flat_leg_is_not_a_position() {
    let flat = r#"{"marginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"1.0","assetPositions":[{"type":"oneWay","position":{"coin":"BTC","szi":"0.0","leverage":{"type":"cross","value":5},"entryPx":null,"positionValue":"0.0","unrealizedPnl":"0.0","marginUsed":"0.0","maxLeverage":40}}]}"#;
    let perp: ClearinghouseState = read(flat);
    assert_eq!(perp.positions().count(), 0);
    assert!(perp.holds_anything(), "it holds a dollar");
    let left_out = flat.replace(r#""totalNtlPos":"0.0","#, "");
    assert!(
        serde_json::from_str::<ClearinghouseState>(&left_out).is_err(),
        "a figure left out is refused, not read as nothing"
    );
}

#[test]
fn one_dex_is_asked_for_by_name_and_the_main_dex_by_none() {
    let user = address(UNIFIED);
    assert_eq!(
        clearinghouse_state_request(user, Dex::Hip3("xyz")),
        format!(r#"{{"type":"clearinghouseState","user":"{UNIFIED}","dex":"xyz"}}"#)
    );
    assert_eq!(
        clearinghouse_state_request(user, Dex::Main),
        format!(r#"{{"type":"clearinghouseState","user":"{UNIFIED}"}}"#)
    );
}
