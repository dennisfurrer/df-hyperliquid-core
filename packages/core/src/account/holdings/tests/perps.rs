//! `clearinghouseState` as the venue writes it: the unified account's main dex and a dex it never
//! traded on, and a flat leg made by hand in the venue's whole shape. The read runs in the mocked
//! journeys.

use super::*;
use crate::account::fixtures::{PERP_UNIFIED, PERP_UNIFIED_XYZ, dec, read};

/// A dollar of margin and a BTC leg closed to zero, still listed.
const FLAT: &str = r#"{"marginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMarginSummary":{"accountValue":"1.0","totalNtlPos":"0.0","totalRawUsd":"1.0","totalMarginUsed":"0.0"},"crossMaintenanceMarginUsed":"0.0","withdrawable":"1.0","assetPositions":[{"type":"oneWay","position":{"coin":"BTC","szi":"0.0","leverage":{"type":"cross","value":5},"entryPx":null,"positionValue":"0.0","unrealizedPnl":"0.0","marginUsed":"0.0","maxLeverage":40}}]}"#;

#[test]
fn a_clearinghouse_state_reads_every_position_and_its_margin_and_leaves_a_flat_leg_out() {
    let perp: ClearinghouseState = read(PERP_UNIFIED);
    let summary = &perp.margin_summary;
    assert_eq!(
        (summary.account_value, summary.total_ntl_pos, summary.total_margin_used),
        (dec("549463.356168"), dec("2401316.1375199999"), dec("480263.227504"))
    );
    assert_eq!((perp.withdrawable, perp.cross_maintenance_margin_used), (dec("7046.148664"), dec("44258.806876")));
    assert_eq!(perp.time, Some(1_791_480_973_789));
    let coins: Vec<&str> = perp.positions().map(|held| held.coin.as_str()).collect();
    assert_eq!(coins, ["BTC", "ETH", "NEAR"]);
    let btc = perp.position("BTC").unwrap();
    assert_eq!((btc.szi, btc.size(), btc.closing_side()), (dec("-22"), dec("22"), Some(Side::Buy)));
    assert!(btc.is_short() && !btc.is_long());
    assert_eq!((btc.entry_px, btc.liquidation_px), (Some(dec("81134.2")), Some(dec("136313.4115825814"))));
    assert_eq!(btc.mark(), Some(dec("80870")), "its value over its size");
    assert_eq!((btc.leverage.mode(), btc.leverage.value(), btc.leverage.raw_usd()), (MarginMode::Cross, 5, None));
    assert_eq!(btc.cum_funding.as_ref().map(|funding| funding.all_time), Some(dec("374.614429")));
    assert!(perp.holds_anything());
    let never: ClearinghouseState = read(PERP_UNIFIED_XYZ);
    assert!(!never.holds_anything(), "a dex never traded on");

    let flat: ClearinghouseState = read(FLAT);
    let leg = &flat.asset_positions[0].position;
    assert_eq!((flat.positions().count(), leg.closing_side(), leg.mark()), (0, None, None));
    assert!(flat.holds_anything(), "it holds a dollar");
}

/// Every recorded clearinghouse, read or sent, carries its cross summary, its cross maintenance and
/// each position's value and leverage whole: one without them is refused, never read as zero.
#[test]
fn a_clearinghouse_missing_what_every_recording_carries_is_refused() {
    let mut whole: serde_json::Value = serde_json::from_str(PERP_UNIFIED).unwrap();
    let isolated = serde_json::json!({"type": "isolated", "value": 5, "rawUsd": "2134968.0"});
    whole["assetPositions"][0]["position"]["leverage"] = isolated;
    let read: ClearinghouseState = serde_json::from_value(whole.clone()).unwrap();
    assert_eq!(read.asset_positions[0].position.leverage.raw_usd(), Some(dec("2134968")));
    let position = "/assetPositions/0/position";
    for (at, gone) in [
        ("", "crossMarginSummary"),
        ("", "crossMaintenanceMarginUsed"),
        ("/marginSummary", "totalNtlPos"),
        (position, "positionValue"),
        (position, "maxLeverage"),
        ("/assetPositions/0/position/leverage", "rawUsd"),
    ] {
        let mut without = whole.clone();
        without.pointer_mut(at).and_then(serde_json::Value::as_object_mut).unwrap().remove(gone).unwrap();
        assert!(serde_json::from_value::<ClearinghouseState>(without).is_err(), "without {at}/{gone}");
    }
}
