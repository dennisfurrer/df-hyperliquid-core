//! Market data every market kind shares: `l2Book`, `candleSnapshot`,
//! `recentTrades` and `allMids`. A coin is `BTC`, `xyz:XYZ100` on a HIP-3
//! dex, `@107` or `PURR/USDC` for a pair, `#14730` for an outcome side.

use std::{collections::BTreeMap, fmt, str::FromStr};

use hl_core::{Address, ApiError, Dec, Transport, info};
use serde::{Deserialize, Deserializer, de};

use crate::{
    order::Side,
    request::{body, dex, number, text},
};

/// How finely a book's levels are bucketed: full precision, or 2 to 5
/// significant figures, and at 5 figures a step of 1, 2 or 5 in the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Precision {
    figures: Option<u32>,
    step: Option<u32>,
}

impl Precision {
    pub const FULL: Precision = Precision { figures: None, step: None };

    /// `nSigFigs`: 2, 3, 4 or 5.
    #[must_use]
    pub const fn figures(figures: u32) -> Option<Precision> {
        match figures {
            2..=5 => Some(Precision { figures: Some(figures), step: None }),
            _ => None,
        }
    }

    /// `mantissa`: 1, 2 or 5, and only at five figures.
    #[must_use]
    pub const fn step(self, step: u32) -> Option<Precision> {
        match (self.figures, step) {
            (Some(5), 1 | 2 | 5) => Some(Precision { figures: Some(5), step: Some(step) }),
            _ => None,
        }
    }
}

/// One price level, the socket's and the read's alike.
pub use hl_core::book::Level;

/// `l2Book`: at most twenty levels a side, best first. hl-core's, the shape the socket sends.
pub type L2Book = hl_core::book::Book;

/// A candle's width, as `candleSnapshot` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interval {
    OneMinute,
    ThreeMinutes,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    TwoHours,
    FourHours,
    EightHours,
    TwelveHours,
    OneDay,
    ThreeDays,
    OneWeek,
    OneMonth,
}

const MINUTE_MS: u64 = 60_000;

impl Interval {
    /// Every interval the venue takes, finest first.
    pub const ALL: [Interval; 14] = [
        Interval::OneMinute,
        Interval::ThreeMinutes,
        Interval::FiveMinutes,
        Interval::FifteenMinutes,
        Interval::ThirtyMinutes,
        Interval::OneHour,
        Interval::TwoHours,
        Interval::FourHours,
        Interval::EightHours,
        Interval::TwelveHours,
        Interval::OneDay,
        Interval::ThreeDays,
        Interval::OneWeek,
        Interval::OneMonth,
    ];

    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Interval::OneMinute => "1m",
            Interval::ThreeMinutes => "3m",
            Interval::FiveMinutes => "5m",
            Interval::FifteenMinutes => "15m",
            Interval::ThirtyMinutes => "30m",
            Interval::OneHour => "1h",
            Interval::TwoHours => "2h",
            Interval::FourHours => "4h",
            Interval::EightHours => "8h",
            Interval::TwelveHours => "12h",
            Interval::OneDay => "1d",
            Interval::ThreeDays => "3d",
            Interval::OneWeek => "1w",
            Interval::OneMonth => "1M",
        }
    }

    /// Its width in milliseconds; a month counts thirty days.
    #[must_use]
    pub const fn ms(self) -> u64 {
        let minutes = match self {
            Interval::OneMinute => 1,
            Interval::ThreeMinutes => 3,
            Interval::FiveMinutes => 5,
            Interval::FifteenMinutes => 15,
            Interval::ThirtyMinutes => 30,
            Interval::OneHour => 60,
            Interval::TwoHours => 120,
            Interval::FourHours => 240,
            Interval::EightHours => 480,
            Interval::TwelveHours => 720,
            Interval::OneDay => 1_440,
            Interval::ThreeDays => 4_320,
            Interval::OneWeek => 10_080,
            Interval::OneMonth => 43_200,
        };
        minutes * MINUTE_MS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an interval; the venue takes 1m 3m 5m 15m 30m 1h 2h 4h 8h 12h 1d 3d 1w 1M")]
pub struct UnknownInterval(pub String);

impl FromStr for Interval {
    type Err = UnknownInterval;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Interval::ALL
            .into_iter()
            .find(|interval| interval.wire() == text)
            .ok_or_else(|| UnknownInterval(text.to_owned()))
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

impl<'de> Deserialize<'de> for Interval {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(de::Error::custom)
    }
}

/// One candle, `t` to `T` inclusive.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Candle {
    #[serde(rename = "t")]
    pub open_time: u64,
    /// The last millisecond inside it, not the next one's open.
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(rename = "s")]
    pub coin: String,
    #[serde(rename = "i")]
    pub interval: Interval,
    #[serde(rename = "o")]
    pub open: Dec,
    #[serde(rename = "c")]
    pub close: Dec,
    #[serde(rename = "h")]
    pub high: Dec,
    #[serde(rename = "l")]
    pub low: Dec,
    /// In the base.
    #[serde(rename = "v")]
    pub volume: Dec,
    #[serde(rename = "n")]
    pub trades: u64,
}

/// A print: `side` is the aggressor's, `users` the buyer then the seller.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MarketTrade {
    pub coin: String,
    pub side: Side,
    pub px: Dec,
    pub sz: Dec,
    pub time: u64,
    pub hash: String,
    pub tid: u64,
    pub users: [Address; 2],
}

/// `allMids`: every coin's mid on one dex. Where a book is empty the venue
/// prints its last trade instead. A mid can sit between two ticks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(transparent)]
pub struct Mids(BTreeMap<String, Dec>);

impl Mids {
    #[must_use]
    pub fn get(&self, coin: &str) -> Option<Dec> {
        self.0.get(coin).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, Dec)> {
        self.0.iter().map(|(coin, mid)| (coin.as_str(), *mid))
    }
}

/// `{"type":"l2Book","coin":..,"nSigFigs"?,"mantissa"?}`.
#[must_use]
pub fn l2_book_request(coin: &str, precision: Precision) -> String {
    let figures = precision.figures.map(|figures| ("nSigFigs", number(u64::from(figures))));
    let step = precision.step.map(|step| ("mantissa", number(u64::from(step))));
    body("l2Book", [Some(("coin", text(coin))), figures, step].into_iter().flatten())
}

/// `{"type":"candleSnapshot","req":{"coin","interval","startTime","endTime"}}`,
/// the times unix milliseconds. Only the 5000 most recent candles are kept.
#[must_use]
pub fn candle_snapshot_request(coin: &str, interval: Interval, start: u64, end: u64) -> String {
    let req = hl_core::Json::Object(vec![
        ("coin".to_owned(), text(coin)),
        ("interval".to_owned(), text(interval.wire())),
        ("startTime".to_owned(), number(start)),
        ("endTime".to_owned(), number(end)),
    ]);
    body("candleSnapshot", [("req", req)])
}

/// `{"type":"recentTrades","coin":..}`.
#[must_use]
pub fn recent_trades_request(coin: &str) -> String {
    body("recentTrades", [("coin", text(coin))])
}

/// `{"type":"allMids","dex"?}`: the main dex, spot and outcomes with it,
/// unless a HIP-3 dex is named.
#[must_use]
pub fn all_mids_request(on: Option<&str>) -> String {
    body("allMids", dex(on))
}

pub async fn l2_book(transport: &impl Transport, coin: &str, precision: Precision) -> Result<L2Book, ApiError> {
    info(transport, l2_book_request(coin, precision)).await
}

/// The candles in the window, oldest first; none when the venue answers `null`.
pub async fn candle_snapshot(
    transport: &impl Transport,
    coin: &str,
    interval: Interval,
    start: u64,
    end: u64,
) -> Result<Vec<Candle>, ApiError> {
    let candles: Option<Vec<Candle>> = info(transport, candle_snapshot_request(coin, interval, start, end)).await?;
    Ok(candles.unwrap_or_default())
}

pub async fn recent_trades(transport: &impl Transport, coin: &str) -> Result<Vec<MarketTrade>, ApiError> {
    info(transport, recent_trades_request(coin)).await
}

pub async fn all_mids(transport: &impl Transport, on: Option<&str>) -> Result<Mids, ApiError> {
    info(transport, all_mids_request(on)).await
}

#[cfg(test)]
#[path = "tests/market.rs"]
mod tests;
