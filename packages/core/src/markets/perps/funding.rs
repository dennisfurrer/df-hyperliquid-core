//! Funding: settled rates (`fundingHistory`), an account's payments (`userFunding`) and the next
//! predicted rates (`predictedFundings`). Both histories answer at most [`FUNDING_PAGE`] rows, so a
//! long range is read in windows ([`funding_windows`]) or pages ([`next_page`]).

use crate::client::{ApiError, Transport, info};
use crate::markets::perps::dex::{Dex, number, request, text};
use crate::utils::dec::MAX_SCALE;
use crate::utils::{Address, Dec, Round};
use serde::Deserialize;

/// A perp settles funding once an hour.
pub const FUNDING_INTERVAL_MS: u64 = 3_600_000;

/// How much settled history one read asks for: twenty days, 480 hourly settlements, under the
/// [`FUNDING_PAGE`] rows the venue answers at most.
pub const FUNDING_WINDOW_MS: u64 = 480 * FUNDING_INTERVAL_MS;

/// The most rows `fundingHistory` and `userFunding` answer at once.
pub const FUNDING_PAGE: usize = 500;

/// Hyperliquid's own name in `predictedFundings`.
pub const HL_PERP: &str = "HlPerp";

/// When the next settlement after `now_ms` is due: the next whole hour.
#[must_use]
pub const fn next_funding(now_ms: u64) -> u64 {
    (now_ms / FUNDING_INTERVAL_MS).saturating_add(1).saturating_mul(FUNDING_INTERVAL_MS)
}

/// `{"type":"fundingHistory","coin":..,"startTime":..}`, and `endTime` when given. A HIP-3 perp
/// is named by its full coin, `xyz:TSLA`.
#[must_use]
pub fn funding_history_request(coin: &str, start_ms: u64, end_ms: Option<u64>) -> String {
    let mut fields = vec![("coin", text(coin)), ("startTime", number(start_ms))];
    fields.extend(end_ms.map(|end| ("endTime", number(end))));
    request("fundingHistory", fields, Dex::Main)
}

/// One perp's settlements from `start_ms`, oldest first, at most [`FUNDING_PAGE`] of them.
pub async fn funding_history(
    transport: &impl Transport,
    coin: &str,
    start_ms: u64,
    end_ms: Option<u64>,
) -> Result<Vec<FundingRate>, ApiError> {
    info(transport, funding_history_request(coin, start_ms, end_ms)).await
}

/// One perp's settlements over a range of any length: one read per [`FUNDING_WINDOW_MS`], oldest
/// first, the answers merged.
pub async fn funding_history_range(
    transport: &impl Transport,
    coin: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<Vec<FundingRate>, ApiError> {
    let mut read = Vec::new();
    for (from, to) in funding_windows(start_ms, end_ms) {
        read.push(funding_history(transport, coin, from, Some(to)).await?);
    }
    Ok(merge_rates(start_ms, end_ms, read))
}

/// `start_ms` to `end_ms`, both inclusive, cut into windows of at most [`FUNDING_WINDOW_MS`],
/// oldest first. Empty when the range is.
#[must_use]
pub fn funding_windows(start_ms: u64, end_ms: u64) -> Vec<(u64, u64)> {
    let mut windows = Vec::new();
    let mut from = start_ms;
    while from <= end_ms {
        let to = from.saturating_add(FUNDING_WINDOW_MS - 1).min(end_ms);
        windows.push((from, to));
        match to.checked_add(1) {
            Some(next) => from = next,
            None => break,
        }
    }
    windows
}

/// One perp's settlements read in windows, as one history: those inside the range, oldest first,
/// each settlement once.
#[must_use]
pub fn merge_rates(start_ms: u64, end_ms: u64, windows: Vec<Vec<FundingRate>>) -> Vec<FundingRate> {
    let mut rates: Vec<FundingRate> =
        windows.into_iter().flatten().filter(|rate| (start_ms..=end_ms).contains(&rate.time)).collect();
    rates.sort_by_key(|rate| rate.time);
    rates.dedup_by_key(|rate| rate.time);
    rates
}

/// One settlement of one perp.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    pub coin: String,
    /// The fraction of a position's value paid for the hour; positive, longs pay.
    pub funding_rate: Dec,
    /// The premium of the book over the oracle the rate came from.
    pub premium: Dec,
    /// Unix milliseconds, just past the hour.
    pub time: u64,
}

/// `{"type":"userFunding","user":..,"startTime":..}`, and `endTime` when given. Every dex's
/// perps answer together.
#[must_use]
pub fn user_funding_request(user: Address, start_ms: u64, end_ms: Option<u64>) -> String {
    let mut fields = vec![("user", text(&user.to_hex())), ("startTime", number(start_ms))];
    fields.extend(end_ms.map(|end| ("endTime", number(end))));
    request("userFunding", fields, Dex::Main)
}

/// An account's settlements from `start_ms`, oldest first, at most [`FUNDING_PAGE`] rows.
pub async fn user_funding(
    transport: &impl Transport,
    user: Address,
    start_ms: u64,
    end_ms: Option<u64>,
) -> Result<Vec<UserFunding>, ApiError> {
    info(transport, user_funding_request(user, start_ms, end_ms)).await
}

/// An account's settlements over a range, reading pages while each comes back full, at most
/// `pages` reads: oldest first, each position's settlement once.
pub async fn user_funding_range(
    transport: &impl Transport,
    user: Address,
    start_ms: u64,
    end_ms: u64,
    pages: usize,
) -> Result<Vec<UserFunding>, ApiError> {
    let mut rows: Vec<UserFunding> = Vec::new();
    let mut from = start_ms;
    for _ in 0..pages {
        let page = user_funding(transport, user, from, Some(end_ms)).await?;
        let next = next_page(from, &page);
        for row in page {
            if !rows.iter().any(|kept| kept.time == row.time && kept.delta.coin == row.delta.coin) {
                rows.push(row);
            }
        }
        match next {
            Some(start) if start <= end_ms => from = start,
            _ => break,
        }
    }
    // A stable sort: rows of one settlement keep the venue's order.
    rows.sort_by_key(|row| row.time);
    Ok(rows)
}

/// Where the page after `page` (read from `start_ms`) starts; `None` when `page` was not full. A
/// full page resumes at its last row's time, since other coins' rows may share it (every hash is
/// zero, so rows are told apart by coin and time), or a millisecond later if that would repeat it.
#[must_use]
pub fn next_page(start_ms: u64, page: &[UserFunding]) -> Option<u64> {
    if page.len() < FUNDING_PAGE {
        return None;
    }
    let last = page.iter().map(|row| row.time).max()?;
    match last > start_ms {
        true => Some(last),
        false => start_ms.checked_add(1),
    }
}

/// One settlement on one of an account's positions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UserFunding {
    /// Unix milliseconds, just past the hour.
    pub time: u64,
    /// All zeros on every funding row.
    pub hash: String,
    pub delta: FundingDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingDelta {
    /// `funding`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `BTC`, or `xyz:XYZ100` on a HIP-3 dex.
    pub coin: String,
    /// What the account received, in the dex's collateral: negative, it paid.
    pub usdc: Dec,
    /// The position settled, signed.
    pub szi: Dec,
    pub funding_rate: Dec,
    #[serde(default)]
    pub n_samples: Option<u32>,
}

/// `{"type":"predictedFundings"}`.
#[must_use]
pub fn predicted_fundings_request() -> String {
    request("predictedFundings", Vec::new(), Dex::Main)
}

pub async fn predicted_fundings(transport: &impl Transport) -> Result<PredictedFundings, ApiError> {
    info(transport, predicted_fundings_request()).await
}

/// `predictedFundings`: for each main-dex perp, each venue's next funding, `null` where that venue
/// lists no such perp. Venues are named as sent: [`HL_PERP`], `BinPerp`, `BybitPerp`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct PredictedFundings(pub Vec<(String, Vec<VenueFunding>)>);

/// One venue's line for a coin: its name and its next funding, if it lists one.
pub type VenueFunding = (String, Option<PredictedFunding>);

impl PredictedFundings {
    /// The coins, in the order sent.
    pub fn coins(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(coin, _)| coin.as_str())
    }

    /// Every venue's line for `coin`.
    #[must_use]
    pub fn venues(&self, coin: &str) -> Option<&[VenueFunding]> {
        self.0.iter().find(|(listed, _)| listed == coin).map(|(_, venues)| venues.as_slice())
    }

    /// One venue's next funding for `coin`.
    #[must_use]
    pub fn get(&self, coin: &str, venue: &str) -> Option<&PredictedFunding> {
        self.venues(coin)?.iter().find(|(listed, _)| listed == venue)?.1.as_ref()
    }

    /// This venue's next funding for `coin`.
    #[must_use]
    pub fn hyperliquid(&self, coin: &str) -> Option<&PredictedFunding> {
        self.get(coin, HL_PERP)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictedFunding {
    /// Over the venue's own interval, never rescaled to an hour.
    pub funding_rate: Dec,
    /// Unix milliseconds.
    pub next_funding_time: u64,
    /// Left out by some venues on some perps.
    #[serde(default)]
    pub funding_interval_hours: Option<u32>,
}

impl PredictedFunding {
    /// The rate over one hour; `None` where the interval is not given.
    #[must_use]
    pub fn hourly(&self) -> Option<Dec> {
        let hours = self.funding_interval_hours.filter(|hours| *hours > 0)?;
        self.funding_rate.checked_div(Dec::from_int(i64::from(hours)), MAX_SCALE, Round::Nearest)
    }
}
