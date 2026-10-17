//! Candles and their intervals, the same shape from `candleSnapshot` and from the `candle` feed.

use std::{borrow::Cow, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

use crate::utils::dec::Dec;

/// A candle's width; every interval the venue accepts.
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
    /// Every interval the venue accepts, finest first.
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

    /// As the venue spells it: `1m` is a minute, `1M` a month.
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

    /// Accepts only the venue's exact spellings.
    #[must_use]
    pub fn parse(text: &str) -> Option<Interval> {
        Interval::ALL.into_iter().find(|interval| interval.wire() == text)
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
        Interval::parse(text).ok_or_else(|| UnknownInterval(text.to_owned()))
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

impl<'de> Deserialize<'de> for Interval {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

/// One candle, `t` to `T` inclusive. The feed resends it as it forms.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Candle {
    /// Open time, milliseconds.
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
    /// In the base asset.
    #[serde(rename = "v")]
    pub volume: Dec,
    /// Number of trades in the bar.
    #[serde(rename = "n")]
    pub trades: u64,
}

#[cfg(test)]
#[path = "tests/candle.rs"]
mod tests;
