//! Live answers, mainnet, 2026-10-08: BTC's book at full precision, at five
//! figures in steps of two and at three figures (three levels a side kept),
//! four hourly BTC candles, three BTC prints, and mids on the main dex and on
//! `xyz` (a few coins of each kept).

use hl_core::Dec;

use super::*;
use crate::{
    order::Side,
    tests_support::{Recorded, block_on},
};

const BOOK: &str = include_str!("../../fixtures/l2_book.json");
const BOOK_STEP_TWO: &str = include_str!("../../fixtures/l2_book_five_figures_step_two.json");
const BOOK_THREE: &str = include_str!("../../fixtures/l2_book_three_figures.json");
const CANDLES: &str = include_str!("../../fixtures/candle_snapshot.json");
const TRADES: &str = include_str!("../../fixtures/recent_trades.json");
const MIDS: &str = include_str!("../../fixtures/all_mids.json");
const MIDS_XYZ: &str = include_str!("../../fixtures/all_mids_xyz.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_book_is_asked_for_at_a_precision_the_venue_takes() {
    assert_eq!(l2_book_request("BTC", Precision::FULL), r#"{"type":"l2Book","coin":"BTC"}"#);
    let three = Precision::figures(3).unwrap();
    assert_eq!(l2_book_request("BTC", three), r#"{"type":"l2Book","coin":"BTC","nSigFigs":3}"#);
    let step_two = Precision::figures(5).and_then(|five| five.step(2)).unwrap();
    assert_eq!(l2_book_request("BTC", step_two), r#"{"type":"l2Book","coin":"BTC","nSigFigs":5,"mantissa":2}"#);
    for refused in [0, 1, 6] {
        assert_eq!(Precision::figures(refused), None, "{refused} figures");
    }
    assert_eq!(three.step(2), None, "a step only at five figures");
    for step in [1, 2, 5] {
        assert!(Precision::figures(5).and_then(|five| five.step(step)).is_some(), "{step}");
    }
    assert_eq!(Precision::figures(5).and_then(|five| five.step(3)), None);
}

#[test]
fn a_book_reads_both_sides_best_first() {
    let book: L2Book = serde_json::from_str(BOOK).unwrap();
    assert_eq!((book.coin.as_str(), book.time, book.spread), ("BTC", 1_791_480_825_902, None));
    assert_eq!(book.bids()[0], Level { px: dec("80858"), sz: dec("23.7926"), n: 61 });
    assert_eq!(book.asks()[0].px, dec("80859"));
    assert_eq!(book.touch(), (Some(dec("80858")), Some(dec("80859"))));
    assert_eq!(book.mid(), Some(dec("80858.5")), "the midpoint, exactly");
    let stepped: L2Book = serde_json::from_str(BOOK_STEP_TWO).unwrap();
    assert_eq!((stepped.asks()[0].px, stepped.spread), (dec("80860"), Some(dec("1"))));
    let three: L2Book = serde_json::from_str(BOOK_THREE).unwrap();
    assert_eq!((three.bids()[0].px, three.bids()[0].n), (dec("80800"), 383));
}

#[test]
fn intervals_are_the_fourteen_the_venue_names() {
    let wire: Vec<&str> = Interval::ALL.iter().map(|interval| interval.wire()).collect();
    assert_eq!(wire, ["1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h", "1d", "3d", "1w", "1M"]);
    for interval in Interval::ALL {
        assert_eq!(interval.wire().parse::<Interval>(), Ok(interval));
    }
    assert!("1y".parse::<Interval>().is_err());
    assert_eq!(Interval::OneHour.ms(), 3_600_000);
}

#[test]
fn candles_are_asked_for_by_coin_interval_and_window() {
    assert_eq!(
        candle_snapshot_request("BTC", Interval::OneHour, 1_791_468_000_000, 1_791_478_800_000),
        r#"{"type":"candleSnapshot","req":{"coin":"BTC","interval":"1h","startTime":1791468000000,"endTime":1791478800000}}"#
    );
    let candles: Vec<Candle> = serde_json::from_str(CANDLES).unwrap();
    assert_eq!(candles.len(), 4);
    let first = &candles[0];
    assert_eq!(
        (first.open_time, first.close_time, first.interval),
        (1_791_468_000_000, 1_791_471_599_999, Interval::OneHour)
    );
    assert_eq!(
        (first.open, first.high, first.low, first.close),
        (dec("82164"), dec("82767"), dec("82105"), dec("82678"))
    );
    assert_eq!((first.coin.as_str(), first.volume, first.trades), ("BTC", dec("1640.54962"), 22_548));
}

#[test]
fn a_print_names_the_aggressor_and_both_sides() {
    assert_eq!(recent_trades_request("BTC"), r#"{"type":"recentTrades","coin":"BTC"}"#);
    let trades: Vec<MarketTrade> = serde_json::from_str(TRADES).unwrap();
    let first = &trades[0];
    assert_eq!(
        (first.side, first.px, first.sz, first.tid),
        (Side::Buy, dec("80839"), dec("0.03885"), 739_564_670_599_029)
    );
    assert_eq!(first.users[0], "0xc861d75087be18a9f46c1dee457bb0e29e31ddbd".parse().unwrap());
    assert_eq!(trades[1].side, Side::Sell);
}

#[test]
fn mids_are_read_per_dex() {
    assert_eq!(all_mids_request(None), r#"{"type":"allMids"}"#);
    assert_eq!(all_mids_request(Some("xyz")), r#"{"type":"allMids","dex":"xyz"}"#);
    let mids: Mids = serde_json::from_str(MIDS).unwrap();
    assert_eq!(mids.get("BTC"), Some(dec("80847.5")));
    assert_eq!(mids.get("@107"), Some(dec("83.524")));
    assert_eq!(mids.get("#14730"), Some(dec("0.612995")), "an outcome's mid, finer than its grid");
    assert_eq!(mids.get("DOGE"), None);
    let xyz: Mids = serde_json::from_str(MIDS_XYZ).unwrap();
    assert_eq!(xyz.get("xyz:AAPL"), Some(dec("338.28")));
    assert_eq!(xyz.len(), 3);
}

#[test]
fn each_read_goes_through_the_transport_once() {
    let transport = Recorded::answering(BOOK);
    assert!(block_on(l2_book(&transport, "BTC", Precision::FULL)).is_ok());
    assert_eq!(transport.sent(), [("/info".to_owned(), l2_book_request("BTC", Precision::FULL))]);
    let transport = Recorded::answering(CANDLES);
    assert_eq!(block_on(candle_snapshot(&transport, "BTC", Interval::OneHour, 0, 1)).unwrap().len(), 4);
    let transport = Recorded::answering("null");
    assert_eq!(block_on(candle_snapshot(&transport, "BTC", Interval::OneHour, 0, 1)).unwrap(), [], "null is none");
    let transport = Recorded::answering(TRADES);
    assert_eq!(block_on(recent_trades(&transport, "BTC")).unwrap().len(), 3);
    let transport = Recorded::answering(MIDS);
    assert_eq!(block_on(all_mids(&transport, None)).unwrap().len(), 6);
}
