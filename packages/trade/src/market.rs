//! Market data every market kind shares: `l2Book`, `candleSnapshot`,
//! `recentTrades` and `allMids`. A coin is `BTC`, `xyz:XYZ100` on a HIP-3
//! dex, `@107` or `PURR/USDC` for a pair, `#14730` for an outcome side.

use hl_core::{ApiError, Transport, info};

use crate::request::{body, dex, number, text};

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

/// A candle and its width: hl-core's, the shape the socket sends.
pub use hl_core::candle::{Candle, Interval, UnknownInterval};

/// A print: `side` is the aggressor's. hl-core's, the shape the socket sends.
pub type MarketTrade = hl_core::trade::Trade;

/// `allMids`: every coin's mid on one dex. hl-core's, the map the socket sends.
pub use hl_core::mids::Mids;

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
