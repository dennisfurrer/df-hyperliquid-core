//! A recorded `candleSnapshot` bar (BTC, an hour) and a recorded `candle` frame (BTC, a minute),
//! mainnet, 2026-10-08, and four hourly BTC candles read live the same day.

use super::*;
use crate::trade::tests_support::{Recorded, block_on};

const CANDLES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/candle_snapshot.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

const READ: &str = r#"{"t":1791468000000,"T":1791471599999,"s":"BTC","i":"1h","o":"82164.0","c":"82678.0","h":"82767.0","l":"82105.0","v":"1640.54962","n":22548}"#;
const SENT: &str = r#"{"t":1791480420000,"T":1791480479999,"s":"BTC","i":"1m","o":"80429.0","c":"80459.0","h":"80470.0","l":"80416.0","v":"30.8389","n":241}"#;

#[test]
fn a_bar_ends_a_millisecond_before_the_next_opens() {
    for (text, interval) in [(READ, Interval::OneHour), (SENT, Interval::OneMinute)] {
        let candle: Candle = serde_json::from_str(text).unwrap();
        assert_eq!(candle.interval, interval);
        assert_eq!(candle.close_time - candle.open_time + 1, interval.ms(), "{interval}");
    }
}

#[test]
fn every_interval_the_venue_names_reads_and_writes_back() {
    let listed = ["1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h", "1d", "3d", "1w", "1M"];
    assert_eq!(Interval::ALL.map(Interval::wire), listed);
    for text in listed {
        assert_eq!(Interval::parse(text).map(Interval::wire), Some(text));
        assert_eq!(text.parse::<Interval>().map(|interval| interval.to_string()), Ok(text.to_owned()));
    }
    for refused in ["7m", "1M ", "1H", "", "60m", "1y"] {
        assert_eq!(Interval::parse(refused), None, "{refused:?}");
        assert_eq!(refused.parse::<Interval>(), Err(UnknownInterval(refused.to_owned())));
    }
    assert!(serde_json::from_str::<Interval>(r#""1y""#).is_err());
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
    let transport = Recorded::answering(CANDLES);
    assert_eq!(block_on(candle_snapshot(&transport, "BTC", Interval::OneHour, 0, 1)).unwrap().len(), 4);
    let transport = Recorded::answering("null");
    assert_eq!(block_on(candle_snapshot(&transport, "BTC", Interval::OneHour, 0, 1)).unwrap(), [], "null is none");
}
