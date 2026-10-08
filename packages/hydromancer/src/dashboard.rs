//! The outcome builder dashboard (`GET /dashboard/outcome-builder-*`, [`crate::DOCS`]): volume,
//! fees, fills and traders on outcome markets, per builder, all time, by category, by hour and by
//! day, and what a builder's traders made. Every decimal comes as a string and reads as a
//! [`Dec`]. A builder is an address, or `None` for the fills no builder routed (the provider
//! writes that bucket `null`, and some routes write it as the string `"null"`).

use std::fmt;

use hl_core::{Address, Dec, Http};
use serde::{Deserialize, Deserializer, de};

use crate::client::{Call, Error, Hydromancer};

/// The most builders a daily answer ranks.
pub const MAX_BUILDERS: u16 = 100;

/// The longest hourly window, in hours.
pub const MAX_HOURS: u16 = 168;

/// The longest daily window, in days. A request for all time asks for one day less.
pub const MAX_DAYS: u32 = 365;

/// How the provider names the bucket of fills no builder routed.
const NO_BUILDER: &str = "null";

fn builder_param(builder: Option<Address>) -> String {
    builder.map_or_else(|| NO_BUILDER.to_owned(), Address::to_hex)
}

/// `outcome-builder-stats?builder=`.
#[must_use]
pub fn stats(builder: Option<Address>) -> Call {
    Call::route("outcome-builder-stats", &[("builder", &builder_param(builder))])
}

/// `outcome-builder-category-stats?builder=`.
#[must_use]
pub fn categories(builder: Option<Address>) -> Call {
    Call::route("outcome-builder-category-stats", &[("builder", &builder_param(builder))])
}

/// `outcome-builder-hourly?builder=&hours=`, the hours held to one through [`MAX_HOURS`].
#[must_use]
pub fn hourly(builder: Option<Address>, hours: u16) -> Call {
    let hours = hours.clamp(1, MAX_HOURS).to_string();
    Call::route("outcome-builder-hourly", &[("builder", &builder_param(builder)), ("hours", &hours)])
}

/// `outcome-builder-volume-daily`, packed: every ranked builder a row, every day a column, from
/// `start` (`YYYY-MM-DD`) to `end` or today, the top `limit` builders (one through
/// [`MAX_BUILDERS`]).
#[must_use]
pub fn volume_daily(start: &str, end: Option<&str>, limit: u16) -> Call {
    let limit = limit.clamp(1, MAX_BUILDERS).to_string();
    let mut query = vec![("startDate", start)];
    if let Some(end) = end {
        query.push(("endDate", end));
    }
    query.extend([("limitBuilders", limit.as_str()), ("format", "packed")]);
    Call::route("outcome-builder-volume-daily", &query)
}

/// `outcome-builder-profitability?builder=&window=`.
#[must_use]
pub fn profitability(builder: Option<Address>, span: Span) -> Call {
    Call::route("outcome-builder-profitability", &[("builder", &builder_param(builder)), ("window", span.as_str())])
}

/// A profitability window. The whole-day spans end yesterday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Span {
    Today,
    Day,
    Week,
    Month,
    Quarter,
    All,
}

impl Span {
    /// As the route spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Span::Today => "today",
            Span::Day => "1d",
            Span::Week => "7d",
            Span::Month => "30d",
            Span::Quarter => "90d",
            Span::All => "all",
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<H: Http> Hydromancer<H> {
    pub async fn builder_stats(&self, builder: Option<Address>) -> Result<BuilderStats, Error> {
        self.read(&stats(builder)).await
    }

    pub async fn categories(&self, builder: Option<Address>) -> Result<CategoryStats, Error> {
        self.read(&categories(builder)).await
    }

    pub async fn hourly(&self, builder: Option<Address>, hours: u16) -> Result<Hourly, Error> {
        self.read(&hourly(builder, hours)).await
    }

    pub async fn volume_daily(&self, start: &str, end: Option<&str>, limit: u16) -> Result<VolumeDaily, Error> {
        self.read(&volume_daily(start, end, limit)).await
    }

    pub async fn profitability(&self, builder: Option<Address>, span: Span) -> Result<Profitability, Error> {
        self.read(&profitability(builder, span)).await
    }
}

/// A builder's all-time outcome totals.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderStats {
    #[serde(deserialize_with = "builder")]
    pub builder: Option<Address>,
    pub builder_fee: Dec,
    /// Notional, in the quote.
    pub volume: Dec,
    /// A fraction of all outcome volume: `0.1691` is 16.91%.
    pub volume_share: Dec,
    pub fills: u64,
    pub unique_users: u64,
}

/// A builder's all-time totals split by market category, the busiest first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStats {
    #[serde(deserialize_with = "builder")]
    pub builder: Option<Address>,
    /// Absent for a builder with no activity.
    #[serde(default)]
    pub overall: Option<Overall>,
    #[serde(default)]
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Overall {
    pub trading_days: u32,
    pub unique_markets: u32,
    pub max_markets_day: BusiestDay,
}

/// The UTC day with the most distinct markets traded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BusiestDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub markets: u32,
}

/// One category. Its shares are of the builder's own totals, not the network's: volume and fill
/// shares add up to one across categories, trader shares can pass it (a trader in two
/// categories counts in both).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    /// `crypto` for a market with no tag.
    pub category: String,
    pub builder_fee: Dec,
    pub volume: Dec,
    pub volume_share: Dec,
    pub fills: u64,
    #[serde(default)]
    pub fills_share: Option<Dec>,
    pub unique_users: u64,
    #[serde(default)]
    pub unique_users_share: Option<Dec>,
    #[serde(default)]
    pub trading_days: Option<u32>,
    /// One outcome is one market.
    pub unique_markets: u32,
    #[serde(default)]
    pub max_markets_day: Option<BusiestDay>,
}

/// A builder's last hours, one figure an hour on every series.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hourly {
    #[serde(deserialize_with = "builder")]
    pub builder: Option<Address>,
    /// `YYYY-MM-DD HH:00:00`, UTC.
    #[serde(default)]
    pub start_hour: Option<String>,
    #[serde(default)]
    pub end_hour: Option<String>,
    /// `YYYY-MM-DD HH:00:00`, UTC, oldest first.
    pub hours: Vec<String>,
    pub series: HourlySeries,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlySeries {
    pub volume: Vec<Dec>,
    pub fills: Vec<u64>,
    pub active_users: Vec<u64>,
    /// Traders routing through the builder for the first time.
    pub new_users: Vec<u64>,
    /// The largest single trade's notional; empty where the answer does not carry it.
    #[serde(default)]
    pub biggest_notional_fill: Vec<Dec>,
}

impl Hourly {
    /// Every series has one figure for every hour. A short one would draw as an hour that traded
    /// nothing, so it is refused rather than drawn.
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        let hours = self.hours.len();
        let series = &self.series;
        series.volume.len() == hours
            && series.fills.len() == hours
            && series.active_users.len() == hours
            && series.new_users.len() == hours
            && (series.biggest_notional_fill.is_empty() || series.biggest_notional_fill.len() == hours)
    }
}

/// The top builders by day, packed: each series is indexed `[builder][day]`, aligned to
/// `builders` and `dates`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDaily {
    pub start_date: String,
    pub end_date: String,
    /// `None` is the bucket of fills no builder routed.
    #[serde(deserialize_with = "builders")]
    pub builders: Vec<Option<Address>>,
    /// `YYYY-MM-DD`, oldest first.
    pub dates: Vec<String>,
    pub series: DailySeries,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySeries {
    pub volume: Vec<Vec<Dec>>,
    pub fills: Vec<Vec<u64>>,
    pub unique_users: Vec<Vec<u64>>,
    pub builder_fee: Vec<Vec<Dec>>,
}

impl VolumeDaily {
    /// One row a builder and one column a day on every series. A short row would under-report the
    /// builder it belongs to, so it is refused rather than folded short.
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        let (rows, days) = (self.builders.len(), self.dates.len());
        let square = |lengths: Vec<usize>| lengths.len() == rows && lengths.iter().all(|&length| length == days);
        let series = &self.series;
        square(series.volume.iter().map(Vec::len).collect())
            && square(series.builder_fee.iter().map(Vec::len).collect())
            && square(series.fills.iter().map(Vec::len).collect())
            && square(series.unique_users.iter().map(Vec::len).collect())
    }
}

/// Realised profit and loss of a builder's traders over completed outcome trades (a position
/// opened and closed) in one window. Ratios are fractions of one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profitability {
    #[serde(deserialize_with = "builder")]
    pub builder: Option<Address>,
    pub window: String,
    pub start_date: String,
    pub end_date: String,
    pub days: u32,
    /// `outcome`: this lineage never overlaps the perp one.
    pub scope: String,
    pub trades: u64,
    pub wins: u64,
    /// An even trade counts as a loss.
    pub losses: u64,
    /// Winning trades over all trades.
    pub win_rate: Dec,
    pub traders: u64,
    pub profitable_traders: u64,
    /// Traders whose summed result is positive, over all traders.
    pub profitable_trader_share: Dec,
    /// Before fees.
    pub gross_pnl: Dec,
    /// After fees.
    pub net_pnl: Dec,
    /// Always zero: an outcome holding accrues no funding.
    pub funding_pnl: Dec,
    pub fees: Dec,
    pub avg_trade_pnl: Dec,
    pub avg_trader_pnl: Dec,
    pub fills: u64,
    pub avg_fills_per_trade: Dec,
    /// `None` when the window holds no completed trade.
    #[serde(default)]
    pub hold_time: Option<HoldTime>,
}

/// How long positions stayed open, in seconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldTime {
    pub mean_seconds: Dec,
    pub p10_seconds: Dec,
    pub p50_seconds: Dec,
    pub p95_seconds: Dec,
}

/// A builder as an answer names it: an address, or `null` (or `"null"`) for no builder.
struct Bucket(Option<Address>);

impl<'de> Deserialize<'de> for Bucket {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let said = Option::<String>::deserialize(deserializer)?;
        match said {
            None => Ok(Bucket(None)),
            Some(text) if text.eq_ignore_ascii_case(NO_BUILDER) => Ok(Bucket(None)),
            Some(text) => text.parse().map(|address| Bucket(Some(address))).map_err(de::Error::custom),
        }
    }
}

fn builder<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Address>, D::Error> {
    Bucket::deserialize(deserializer).map(|bucket| bucket.0)
}

fn builders<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Option<Address>>, D::Error> {
    Vec::<Bucket>::deserialize(deserializer).map(|buckets| buckets.into_iter().map(|bucket| bucket.0).collect())
}

#[cfg(test)]
#[path = "tests/dashboard.rs"]
mod tests;
