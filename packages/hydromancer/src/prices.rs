//! `perpPriceHistoryByTime` in its batch form: the mark and oracle prices the venue accepted for
//! each perp, one point an oracle round (about every three seconds a dex), oldest first, on the
//! native dex and every HIP-3 dex. These are the prices margin, liquidation and funding use.

use std::collections::BTreeMap;

use hl_core::{Dec, Http, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::client::{Call, Error, Hydromancer};

const REQUEST: &str = "perpPriceHistoryByTime";

/// Coins one request may name.
pub const MAX_COINS: usize = 20;

/// Points a coin, at most, in the batch form, counted forward from the start: a window that
/// reaches it loses its newest points.
pub const MAX_POINTS: usize = 100;

/// About how far apart two rounds are, in milliseconds.
pub const TYPICAL_GAP_MS: u64 = 3_000;

/// One accepted round for one coin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricePoint {
    /// The block's time, Unix milliseconds.
    pub time: u64,
    /// The block that committed the round.
    pub block: u64,
    /// `hyperliquid` for the native dex.
    pub dex: String,
    pub coin: String,
    pub mark: Dec,
    /// Absent where the round carried no oracle slot for the asset.
    pub oracle: Option<Dec>,
    /// The external perp reference, where the asset has one.
    pub external: Option<Dec>,
    /// HIP-3 only: how the round was sourced (`Normal`, `Deployer`, `Fallback`).
    pub class: Option<String>,
}

/// Each coin's points, oldest first.
pub type History = BTreeMap<String, Vec<PricePoint>>;

/// A window read for a set of coins: what came back, and for each coin that has nothing, why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceWindow {
    pub history: History,
    pub errors: Vec<(String, String)>,
}

/// `{"type":"perpPriceHistoryByTime","coins":[...],"startTime":..,"endTime":..}`, times in Unix
/// milliseconds. Name at most [`MAX_COINS`] coins.
#[must_use]
pub fn history(coins: &[&str], start: u64, end: u64) -> Call {
    let coins = coins.iter().map(|coin| Json::String((*coin).to_owned())).collect();
    Call::info(&Json::Object(vec![
        ("type".to_owned(), Json::String(REQUEST.to_owned())),
        ("coins".to_owned(), Json::Array(coins)),
        ("startTime".to_owned(), Json::Number(start.to_string())),
        ("endTime".to_owned(), Json::Number(end.to_string())),
    ]))
}

/// The batch answer, keyed by coin. A point that does not read (no time, no mark price, an
/// empty one, some other shape) is dropped alone: one bad element is not a lost window.
pub fn read_history(body: &[u8]) -> Result<History, String> {
    let raw: BTreeMap<String, Vec<Value>> = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    Ok(raw
        .into_iter()
        .map(|(coin, points)| (coin.clone(), points.into_iter().filter_map(|point| read_point(&coin, point)).collect()))
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPoint {
    time: u64,
    #[serde(default)]
    block_number: Option<u64>,
    #[serde(default)]
    dex: Option<String>,
    #[serde(default)]
    coin: Option<String>,
    mark_px: String,
    #[serde(default)]
    oracle_px: Option<String>,
    #[serde(default)]
    ext_perp_px: Option<String>,
    #[serde(default)]
    update_class: Option<String>,
}

fn read_point(coin: &str, raw: Value) -> Option<PricePoint> {
    let raw: RawPoint = serde_json::from_value(raw).ok()?;
    let figure = |said: Option<String>| said.and_then(|said| Dec::parse(&said).ok());
    Some(PricePoint {
        time: raw.time,
        block: raw.block_number.unwrap_or_default(),
        dex: raw.dex.unwrap_or_default(),
        coin: raw.coin.unwrap_or_else(|| coin.to_owned()),
        mark: Dec::parse(&raw.mark_px).ok()?,
        oracle: figure(raw.oracle_px),
        external: figure(raw.ext_perp_px),
        class: raw.update_class,
    })
}

/// A window from an answer or its failure: a failure marks every coin, and a coin the answer
/// skipped or left empty is named rather than shown as a price that did not move.
#[must_use]
pub fn window(coins: &[&str], answer: Result<History, Error>) -> PriceWindow {
    match answer {
        Ok(mut history) => {
            let mut errors = Vec::new();
            for coin in coins {
                if history.get(*coin).is_none_or(Vec::is_empty) {
                    history.remove(*coin);
                    errors.push(((*coin).to_owned(), "the answer carried no readable point for this coin".to_owned()));
                }
            }
            PriceWindow { history, errors }
        }
        Err(error) => {
            let said = error.to_string();
            PriceWindow {
                history: History::new(),
                errors: coins.iter().map(|coin| ((*coin).to_owned(), said.clone())).collect(),
            }
        }
    }
}

/// The last point at or before `at`: the price at that instant. A later point is never it, so
/// an instant before every point has none.
#[must_use]
pub fn point_at_or_before(points: &[PricePoint], at: u64) -> Option<&PricePoint> {
    points.iter().filter(|point| point.time <= at).max_by_key(|point| point.time)
}

/// The `count` points closest to `at`, in time order. A tie goes to the earlier point, the one
/// at or before the instant.
#[must_use]
pub fn nearest(points: &[PricePoint], at: u64, count: usize) -> Vec<&PricePoint> {
    let mut order: Vec<&PricePoint> = points.iter().collect();
    order.sort_by_key(|point| (at.abs_diff(point.time), point.time));
    order.truncate(count);
    order.sort_by_key(|point| point.time);
    order
}

impl<H: Http> Hydromancer<H> {
    /// Each coin's points from `start` to `end`, Unix milliseconds. No coins asks nothing; more
    /// than [`MAX_COINS`] go out in batches of that many, one after another.
    pub async fn price_history(&self, coins: &[&str], start: u64, end: u64) -> Result<History, Error> {
        let mut out = History::new();
        for batch in coins.chunks(MAX_COINS) {
            let call = history(batch, start, end);
            let response = self.send(&call).await?;
            let read = read_history(&response.body).map_err(|detail| self.unreadable(&call, &detail))?;
            out.extend(read);
        }
        Ok(out)
    }

    /// [`Hydromancer::price_history`] as a [`PriceWindow`]: every coin has points or a reason.
    pub async fn price_window(&self, coins: &[&str], start: u64, end: u64) -> PriceWindow {
        window(coins, self.price_history(coins, start, end).await)
    }
}

#[cfg(test)]
#[path = "tests/prices.rs"]
mod tests;
