//! Live fill, order and spot answers as the venue wrote them, and fills made by hand where a rule
//! is the point.

use super::*;
use crate::account::history::fills::Fill as RawFill;
use crate::account::history::orders::OpenOrder as RawOrder;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn raw(coin: &str, side: &str, sz: &str, start: &str, dir: &str, time: u64) -> RawFill {
    RawFill {
        coin: coin.to_owned(),
        px: dec("0.5"),
        sz: dec(sz),
        side: Side::parse(side).unwrap(),
        time,
        start_position: dec(start),
        dir: dir.to_owned(),
        closed_pnl: Dec::ZERO,
        hash: "0x".to_owned(),
        oid: 1,
        crossed: true,
        fee: Dec::ZERO,
        tid: 1,
        cloid: None,
        fee_token: "USDC".to_owned(),
        builder_fee: None,
        liquidation: None,
        twap_id: None,
    }
}

fn read(raws: &[RawFill]) -> Vec<Fill> {
    raws.iter().filter_map(Fill::read).collect()
}

#[test]
fn every_direction_the_venue_sends_is_classified() {
    for text in ["Buy", "Sell"] {
        let direction: Direction = text.parse().unwrap();
        assert!(direction.is_trade() && !direction.is_structural(), "{text}");
    }
    for text in ["Split Outcome", "Merge Outcome", "Negate Outcome", "Merge Question"] {
        let direction: Direction = text.parse().unwrap();
        assert!(direction.is_structural() && !direction.is_trade(), "{text}");
    }
    let settlement: Direction = "Settlement".parse().unwrap();
    assert!(settlement.is_settlement() && !settlement.is_trade() && !settlement.is_structural());
    assert_eq!(Direction::read("Negate Outcome").slug(), "negate_outcome");
}

#[test]
fn an_unknown_direction_survives_verbatim() {
    let direction: Direction = "Auto-Deleveraging".parse().unwrap();
    assert_eq!(direction.slug(), "Auto-Deleveraging");
    assert!(!direction.is_trade() && !direction.is_structural() && !direction.is_settlement());
}

#[test]
fn a_perp_fill_in_the_same_answer_is_skipped_rather_than_refused() {
    let mut perp = raw("BTC", "B", "0.5", "0", "Open Long", 1);
    perp.px = dec("118435.0");
    let fills = read(&[perp, raw("#12100", "B", "10", "0", "Buy", 2)]);
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].coin.to_string(), "#12100");
}

/// Live, 2026-09-24: a builder's buy on `#12270`. Its fee is all builder fee: the venue billed the
/// buy nothing.
#[test]
fn a_live_fill_reads_every_field_the_venue_sent() {
    let text = r##"{
        "coin": "#12270", "px": "0.21349", "sz": "34.0", "side": "B",
        "time": 1789233746112, "startPosition": "1355.0", "dir": "Buy",
        "closedPnl": "0.0", "hash": "0x3263f9", "oid": 543204924246,
        "crossed": true, "fee": "0.00181466", "builderFee": "0.00181466",
        "tid": 881794509642454, "cloid": "0x798cf5", "feeToken": "USDC",
        "twapId": null
    }"##;
    let fill = Fill::read(&serde_json::from_str(text).unwrap()).unwrap();
    assert_eq!((fill.coin, fill.side), (Coin::parse("#12270").unwrap(), Side::Buy));
    assert_eq!(fill.price, dec("0.21349"));
    assert_eq!(fill.size, Qty::new(34));
    assert_eq!((fill.start_position, fill.position), (SignedQty::new(1355), SignedQty::new(1389)));
    assert_eq!(fill.fee, dec("0.00181466"));
    assert_eq!(fill.builder_fee, Some(fill.fee));
    assert_eq!(fill.venue_fee(), Some(Dec::ZERO));
    assert_eq!(fill.fee_token, "USDC");
    assert_eq!(fill.notional(), dec("7.25866"));
    assert_eq!((fill.at_ms, fill.oid, fill.tid), (1_789_233_746_112, 543_204_924_246, 881_794_509_642_454));
}

/// Every recorded fill, read or sent, carries its fee token and whether it
/// crossed: a row without them is refused, not read as no token and resting.
#[test]
fn a_fill_without_its_fee_token_or_crossed_is_refused() {
    let text = r##"{"coin": "#12270", "px": "0.21349", "sz": "34.0", "side": "B",
        "time": 1789233746112, "startPosition": "1355.0", "dir": "Buy", "closedPnl": "0.0",
        "hash": "0x3263f9", "oid": 543204924246, "crossed": true, "fee": "0.00181466",
        "tid": 881794509642454, "feeToken": "USDC"}"##;
    assert!(serde_json::from_str::<RawFill>(text).is_ok());
    for gone in [r#", "feeToken": "USDC""#, r#""crossed": true, "#] {
        assert!(serde_json::from_str::<RawFill>(&text.replace(gone, "")).is_err(), "without {gone}");
    }
}

/// A structural fill prices at `1/n`, finer than the grid: it is read, not
/// dropped.
#[test]
fn a_structural_fill_off_the_grid_is_still_read() {
    let mut negate = raw("#12101", "B", "3", "0", "Negate Outcome", 1);
    negate.px = dec("0.333333");
    let fill = Fill::read(&negate).unwrap();
    assert!(fill.direction.is_structural());
    assert_eq!(fill.price, dec("0.333333"));
}

#[test]
fn an_order_reports_what_the_book_has_already_taken() {
    let text = r##"{
        "coin": "#12100", "side": "B", "limitPx": "0.27", "sz": "120.0",
        "oid": 7, "timestamp": 1789233746112, "origSz": "500.0"
    }"##;
    let raw: RawOrder = serde_json::from_str(text).unwrap();
    let order = OpenOrder::read(&raw).unwrap();
    assert_eq!((order.price, order.size, order.side), (Px::parse("0.27").unwrap(), Qty::new(120), Side::Buy));
    assert_eq!(order.filled(), Qty::new(380));
    assert!(OpenOrder::read(&RawOrder { coin: "BTC".to_owned(), ..raw }).is_none());
}

/// Every recorded resting order, read or sent, carries its original size: a
/// row without it is refused, not read as never filled.
#[test]
fn an_order_without_its_original_size_is_refused() {
    let text =
        r##"{"coin": "#12100", "side": "B", "limitPx": "0.27", "sz": "120.0", "oid": 7, "timestamp": 1789233746112}"##;
    assert!(serde_json::from_str::<RawOrder>(text).is_err());
}

/// A live `spotClearinghouseState`: outcome rows spelled `+`, a zero row, a lending row, USDC.
const SPOT: &str = r#"{"balances":[
    {"coin":"USDC","token":0,"total":"425.91783129","hold":"215.02593","entryNtl":"0.0"},
    {"coin":"HYPE","token":150,"total":"0.00251179","hold":"0.0","entryNtl":"0.11632179"},
    {"coin":"o458","total":"0.0","hold":"0.0","entryNtl":"0.0"},
    {"coin":"+12140","total":"218.0","hold":"12.0","entryNtl":"58.975575"},
    {"coin":"+12160","total":"709.0","hold":"0.0","entryNtl":"59.38904265"},
    {"coin":"+27650","total":"0.0","hold":"0.0","entryNtl":"0.0"}],
    "tokenToAvailableAfterMaintenance":[]}"#;

/// Every recorded spot row, read or sent, carries its total, hold and entry
/// notional: a row without one is refused, not read as nothing held.
#[test]
fn a_spot_row_without_its_hold_is_refused() {
    let without = SPOT.replacen(r#""hold":"12.0","#, "", 1);
    assert_ne!(without, SPOT);
    assert!(serde_json::from_str::<SpotState>(&without).is_err());
}

#[test]
fn a_spot_answer_yields_outcome_holdings_and_nothing_else() {
    let state: SpotState = serde_json::from_str(SPOT).unwrap();
    let held = holdings(&state);
    assert_eq!(held.len(), 2);
    let first = &held[0];
    assert_eq!(first.coin, Coin::parse("#12140").unwrap());
    assert_eq!((first.total, first.hold, first.entry_ntl), (dec("218"), dec("12"), dec("58.975575")));
    assert_eq!(first.free(), dec("206"), "hold is part of total and never added to it");
    assert_eq!(first.average(), Some(dec("0.27053016")), "what it cost a contract, to eight places");
    assert_eq!(held[1].coin, Coin::parse("+12160").unwrap());
    assert_eq!(held[1].average(), Some(dec("0.08376452")));
}
