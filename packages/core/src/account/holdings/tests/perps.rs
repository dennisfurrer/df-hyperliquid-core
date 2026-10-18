//! `clearinghouseState` as the venue writes it: the unified account's main dex and a dex it never
//! traded on, a flat leg made by hand in the venue's whole shape, HLP Strategy A
//! (`clearinghouse_hlp_a_mainnet.json`, 2026-10-08, 177 positions cut to four; the summaries are
//! as sent, so they count the positions left out), and an isolated position beside an empty `xyz`
//! from one `allDexsClearinghouseState` frame (2026-09-26). The read runs in the mocked journeys.

use super::*;
use crate::account::Leverage;
use crate::account::fixtures::{PERP_UNIFIED, PERP_UNIFIED_XYZ, dec, read};
use crate::utils::Dec;

const HLP_A: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_hlp_a_mainnet.json"));
const ISOLATED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_isolated_mainnet.json"));
const XYZ_EMPTY: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_xyz_empty_mainnet.json"));

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

/// The venue's position value is the size at the mark, so the mark it implies lands on the price
/// grid: 80735 is a whole BTC price, 0.12713 a five-figure DYDX one.
#[test]
fn a_cross_state_reads_every_figure_and_each_position_its_side_and_mark() {
    let state: ClearinghouseState = read(HLP_A);
    let summary = &state.margin_summary;
    assert_eq!(
        (summary.account_value, summary.total_ntl_pos, summary.total_raw_usd, summary.total_margin_used),
        (dec("3173167.3515860001"), dec("3735007.1955929999"), dec("2920252.4349730001"), dec("186750.359729"))
    );
    assert_eq!(state.cross_margin_summary, state.margin_summary, "all of it cross");
    assert_eq!(
        (state.cross_maintenance_margin_used, state.withdrawable),
        (dec("37350.071885"), dec("2799666.6320639998"))
    );
    assert_eq!(state.time, Some(1_791_480_567_359));
    let btc = state.position("BTC").unwrap();
    assert_eq!((btc.szi, btc.entry_px, btc.position_value), (dec("0.75104"), Some(dec("82511.5")), dec("60635.2144")));
    assert_eq!((btc.unrealized_pnl, btc.return_on_equity), (dec("-1334.284132"), Some(dec("-0.4306260866"))));
    assert_eq!((btc.liquidation_px, btc.margin_used, btc.max_leverage), (None, dec("3031.76072"), 50));
    assert_eq!(
        (btc.leverage.clone(), btc.leverage.mode(), btc.leverage.value()),
        (Leverage::Cross { value: 20 }, MarginMode::Cross, 20)
    );
    let funding = btc.cum_funding.as_ref().unwrap();
    assert_eq!(
        (funding.all_time, funding.since_open, funding.since_change),
        (dec("-612507.411689"), dec("490.399487"), Dec::ZERO)
    );
    let dydx = state.position("DYDX").unwrap();
    assert_eq!((dydx.liquidation_px, dydx.size()), (Some(dec("10.3702027403")), dec("303109.2")));
    let sides = [("BTC", true, Side::Sell), ("DYDX", false, Side::Buy)];
    for (coin, long, closing) in sides {
        let position = state.position(coin).unwrap();
        assert_eq!(
            (position.is_long(), position.is_short(), position.closing_side()),
            (long, !long, Some(closing)),
            "{coin}"
        );
    }
    let marks: Vec<(&str, Dec)> =
        state.positions().map(|position| (position.coin.as_str(), position.mark().unwrap())).collect();
    assert_eq!(
        marks,
        [("BTC", dec("80735")), ("ETH", dec("2419.8")), ("DYDX", dec("0.12713")), ("AVAX", dec("9.8715"))]
    );
}

/// What an isolated position uses is its value plus its `rawUsd`, to the last digit; the account's
/// value is likewise its raw USD plus its notional.
#[test]
fn an_isolated_position_carries_its_raw_usd() {
    let state: ClearinghouseState = read(ISOLATED);
    let btc = state.position("BTC").unwrap();
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    assert_eq!(btc.leverage.mode(), MarginMode::Isolated);
    assert_eq!(btc.position_value.checked_add(btc.leverage.raw_usd().unwrap()), Some(btc.margin_used));
    assert_eq!((btc.liquidation_px, btc.mark()), (Some(dec("63823.6021044955")), Some(dec("83974"))));
    let summary = &state.margin_summary;
    assert_eq!(summary.total_raw_usd.checked_add(summary.total_ntl_pos), Some(summary.account_value));
    assert_eq!(state.cross_margin_summary.account_value, Dec::ZERO, "nothing is cross");
}

/// An empty dex and a subaccount's missing `time` and `returnOnEquity` read as nothing held and
/// unknown, never as a position, 1970 or zero.
#[test]
fn what_is_not_held_or_not_sent_reads_as_nothing() {
    let empty: ClearinghouseState = read(XYZ_EMPTY);
    assert_eq!((empty.positions().count(), empty.holds_anything()), (0, false));
    let mut text: serde_json::Value = serde_json::from_str(XYZ_EMPTY).unwrap();
    text.as_object_mut().unwrap().remove("time");
    assert_eq!(serde_json::from_value::<ClearinghouseState>(text).unwrap().time, None);
    let mut text: serde_json::Value = serde_json::from_str(ISOLATED).unwrap();
    text["assetPositions"][0]["position"].as_object_mut().unwrap().remove("returnOnEquity");
    let state: ClearinghouseState = serde_json::from_value(text).unwrap();
    assert_eq!(state.asset_positions[0].position.return_on_equity, None);
}
