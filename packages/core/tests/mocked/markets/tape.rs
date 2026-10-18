//! A market's tape: its candles over a window, its last prints and its mid.

use df_hyperliquid_core::markets::{Dex, Interval, Side, candle, mids, trades};
use df_hyperliquid_core::{Client, Network};

use super::{bodies, dec};
use crate::support::clock;
use crate::support::venue::Venue;

/// BTC on mainnet, 2026-10-08: four hourly candles to 18:00 UTC, three prints and the mids read
/// the same afternoon. A window the venue keeps nothing for answers `null`.
#[tokio::test]
async fn btc_s_candles_prints_and_mid_are_read_and_an_empty_window_has_no_candles() {
    let empty = r#"{"coin":"BTC","interval":"1m","startTime":0,"endTime":60000}"#;
    let venue = Venue::mainnet()
        .info_where("candleSnapshot", &[("req", empty)], "null")
        .info("candleSnapshot", "trade/candle_snapshot.json")
        .info("recentTrades", "trade/recent_trades.json")
        .info("allMids", "trade/all_mids.json");
    let client = Client::reader(venue, Network::Mainnet, clock);

    let candles =
        candle::candle_snapshot(&client, "BTC", Interval::OneHour, 1_791_468_000_000, 1_791_482_399_999).await.unwrap();
    let prints = trades::recent_trades(&client, "BTC").await.unwrap();
    let mids = mids::all_mids(&client, Dex::Main).await.unwrap();
    let none = candle::candle_snapshot(&client, "BTC", Interval::OneMinute, 0, 60_000).await.unwrap();

    assert_eq!(
        bodies(&client),
        [
            r#"{"type":"candleSnapshot","req":{"coin":"BTC","interval":"1h","startTime":1791468000000,"endTime":1791482399999}}"#,
            r#"{"type":"recentTrades","coin":"BTC"}"#,
            r#"{"type":"allMids"}"#,
            format!(r#"{{"type":"candleSnapshot","req":{empty}}}"#).as_str(),
        ]
    );
    let window: Vec<(u64, u64)> = candles.iter().map(|bar| (bar.open_time, bar.close_time)).collect();
    assert_eq!(window.first(), Some(&(1_791_468_000_000, 1_791_471_599_999)));
    assert_eq!(window.last(), Some(&(1_791_478_800_000, 1_791_482_399_999)), "the bar still forming");
    let forming = &candles[3];
    let mid = mids.get("BTC").unwrap();
    assert!(forming.low <= mid && mid <= forming.high, "{mid} inside {} to {}", forming.low, forming.high);
    let sides: Vec<Side> = prints.iter().map(|print| print.side).collect();
    assert_eq!(sides, [Side::Buy, Side::Sell, Side::Buy]);
    assert!(prints.iter().all(|print| print.coin == "BTC" && print.buyer != print.seller));
    assert_eq!((prints[0].px, mids.get("@107")), (dec("80839"), Some(dec("83.524"))));
    assert!(none.is_empty());
}
