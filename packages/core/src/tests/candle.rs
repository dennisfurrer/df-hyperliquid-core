//! hl-trade's recorded `candleSnapshot` bar (BTC, an hour) and hl-ws's recorded `candle` frame
//! (BTC, a minute), mainnet, 2026-10-08, read into the one candle.

use super::*;

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
