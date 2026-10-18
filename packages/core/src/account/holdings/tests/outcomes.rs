//! Outcome fills, orders and sides read from the rows of every market: live answers as the venue
//! wrote them, and fills made by hand where a rule is the point.

use super::*;
use crate::account::history::fills::Fill as RawFill;
use crate::account::history::orders::OpenOrder as RawOrder;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn raw(coin: &str, px: &str, dir: &str) -> RawFill {
    RawFill {
        coin: coin.to_owned(),
        px: dec(px),
        sz: dec("3"),
        side: Side::Buy,
        time: 1,
        start_position: Dec::ZERO,
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

#[test]
fn every_direction_the_venue_sends_is_classified_and_any_other_kept_verbatim() {
    for (text, slug, trade, structural, settlement) in [
        ("Buy", "buy", true, false, false),
        ("Sell", "sell", true, false, false),
        ("Split Outcome", "split_outcome", false, true, false),
        ("Merge Outcome", "merge_outcome", false, true, false),
        ("Negate Outcome", "negate_outcome", false, true, false),
        ("Merge Question", "merge_question", false, true, false),
        ("Settlement", "settlement", false, false, true),
        ("Auto-Deleveraging", "Auto-Deleveraging", false, false, false),
    ] {
        let direction: Direction = text.parse().unwrap();
        assert_eq!(direction.to_string(), slug, "{text}");
        assert_eq!(
            (direction.is_trade(), direction.is_structural(), direction.is_settlement()),
            (trade, structural, settlement),
            "{text}"
        );
    }
}

/// Live, 2026-09-24: a builder's buy on `#12270`, its fee all builder fee: the venue billed the buy
/// nothing. A structural fill prices at `1/n`, finer than the grid, and is read; a perp fill in
/// the same answer is skipped rather than refused.
#[test]
fn an_outcome_fill_is_read_from_its_row_and_any_other_row_skipped() {
    let text = r##"{
        "coin": "#12270", "px": "0.21349", "sz": "34.0", "side": "B",
        "time": 1789233746112, "startPosition": "1355.0", "dir": "Buy",
        "closedPnl": "0.0", "hash": "0x3263f9", "oid": 543204924246,
        "crossed": true, "fee": "0.00181466", "builderFee": "0.00181466",
        "tid": 881794509642454, "cloid": "0x798cf5", "feeToken": "USDC",
        "twapId": null
    }"##;
    let fill = Fill::read(&serde_json::from_str(text).unwrap()).unwrap();
    assert_eq!((fill.coin, fill.side, fill.price), (Coin::parse("#12270").unwrap(), Side::Buy, dec("0.21349")));
    assert_eq!(
        (fill.size, fill.start_position, fill.position),
        (Qty::new(34), SignedQty::new(1355), SignedQty::new(1389))
    );
    assert_eq!(
        (fill.fee, fill.builder_fee, fill.venue_fee()),
        (dec("0.00181466"), Some(dec("0.00181466")), Some(Dec::ZERO))
    );
    assert_eq!((fill.fee_token.as_str(), fill.notional()), ("USDC", dec("7.25866")));
    assert_eq!((fill.at_ms, fill.oid, fill.tid), (1_789_233_746_112, 543_204_924_246, 881_794_509_642_454));

    let negate = Fill::read(&raw("#12101", "0.333333", "Negate Outcome")).unwrap();
    assert!(negate.direction.is_structural());
    assert_eq!(negate.price, dec("0.333333"), "off the grid, kept");
    assert_eq!(Fill::read(&raw("BTC", "118435.0", "Open Long")), None);
}

#[test]
fn an_outcome_order_reports_what_the_book_has_already_taken() {
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

/// A live `spotClearinghouseState`: outcome rows spelled `+`, a zero row, a lending row, USDC.
#[test]
fn a_spot_answer_yields_outcome_holdings_and_nothing_else() {
    let state: SpotState = serde_json::from_str(
        r#"{"balances":[
            {"coin":"USDC","token":0,"total":"425.91783129","hold":"215.02593","entryNtl":"0.0"},
            {"coin":"HYPE","token":150,"total":"0.00251179","hold":"0.0","entryNtl":"0.11632179"},
            {"coin":"o458","total":"0.0","hold":"0.0","entryNtl":"0.0"},
            {"coin":"+12140","total":"218.0","hold":"12.0","entryNtl":"58.975575"},
            {"coin":"+12160","total":"709.0","hold":"0.0","entryNtl":"59.38904265"},
            {"coin":"+27650","total":"0.0","hold":"0.0","entryNtl":"0.0"}],
            "tokenToAvailableAfterMaintenance":[]}"#,
    )
    .unwrap();
    let held = holdings(&state);
    assert_eq!(held.iter().map(|side| side.coin.to_string()).collect::<Vec<_>>(), ["#12140", "#12160"]);
    let first = &held[0];
    assert_eq!((first.total, first.hold, first.entry_ntl), (dec("218"), dec("12"), dec("58.975575")));
    assert_eq!(first.free(), dec("206"), "hold is part of total and never added to it");
    assert_eq!(first.average(), Some(dec("0.27053016")), "what it cost a contract, to eight places");
    assert_eq!(held[1].average(), Some(dec("0.08376452")));
}
