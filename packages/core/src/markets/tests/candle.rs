//! Four hourly BTC candles read on mainnet, 2026-10-08, and a recorded `candle` frame (BTC, a
//! minute) the same day. Reads through a client are in `tests/mocked/markets`.

use super::*;

const CANDLES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/candle_snapshot.json"));
const SENT: &str = r#"{"t":1791480420000,"T":1791480479999,"s":"BTC","i":"1m","o":"80429.0","c":"80459.0","h":"80470.0","l":"80416.0","v":"30.8389","n":241}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_candle_reads_its_bar_and_ends_a_millisecond_before_the_next_opens() {
    let candles: Vec<Candle> = serde_json::from_str(CANDLES).unwrap();
    assert_eq!(candles.len(), 4);
    let first = &candles[0];
    assert_eq!((first.coin.as_str(), first.open_time, first.interval), ("BTC", 1_791_468_000_000, Interval::OneHour));
    assert_eq!(
        (first.open, first.high, first.low, first.close),
        (dec("82164"), dec("82767"), dec("82105"), dec("82678"))
    );
    assert_eq!((first.volume, first.trades), (dec("1640.54962"), 22_548));
    assert!(candles.windows(2).all(|pair| pair[0].close_time + 1 == pair[1].open_time), "back to back");
    let frame: Candle = serde_json::from_str(SENT).unwrap();
    for candle in candles.iter().chain([&frame]) {
        assert_eq!(candle.close_time - candle.open_time + 1, candle.interval.ms(), "{}", candle.interval);
    }
}

#[test]
fn every_interval_reads_and_writes_as_the_venue_spells_it() {
    let minute = 60_000;
    let table = [
        ("1m", minute),
        ("3m", 3 * minute),
        ("5m", 5 * minute),
        ("15m", 15 * minute),
        ("30m", 30 * minute),
        ("1h", 60 * minute),
        ("2h", 120 * minute),
        ("4h", 240 * minute),
        ("8h", 480 * minute),
        ("12h", 720 * minute),
        ("1d", 1_440 * minute),
        ("3d", 4_320 * minute),
        ("1w", 10_080 * minute),
        ("1M", 43_200 * minute),
    ];
    assert_eq!(Interval::ALL.map(Interval::wire), table.map(|(wire, _)| wire), "finest first");
    for (interval, (wire, ms)) in Interval::ALL.into_iter().zip(table) {
        assert_eq!((interval.wire(), interval.ms(), interval.to_string()), (wire, ms, wire.to_owned()));
        assert_eq!(wire.parse::<Interval>(), Ok(interval));
        assert_eq!(serde_json::from_str::<Interval>(&format!("\"{wire}\"")).unwrap(), interval);
    }
    for refused in ["7m", "1M ", "1H", "", "60m", "1y"] {
        assert_eq!(refused.parse::<Interval>(), Err(UnknownInterval(refused.to_owned())), "{refused:?}");
    }
    assert!(serde_json::from_str::<Interval>(r#""1y""#).is_err());
}
