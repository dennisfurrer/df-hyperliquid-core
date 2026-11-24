//! `l2Book`: a market's visible book, the same shape from `/info` and from the socket, and the
//! precision it is asked at. Prices and sizes are exact [`Dec`] values read from the venue's strings.

use serde::Deserialize;

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::Json;
use crate::utils::dec::{Dec, Round};

/// How finely a book's levels are bucketed: full precision, or 2 to 5
/// significant figures, and at 5 figures a step of 1, 2 or 5 in the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Precision {
    figures: Option<u32>,
    step: Option<u32>,
}

impl Precision {
    /// Full precision: no `nSigFigs`, no `mantissa`.
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

/// `{"type":"l2Book","coin":..,"nSigFigs"?,"mantissa"?}`. A coin is `BTC`, `xyz:XYZ100` on a HIP-3
/// dex, `@107` or `PURR/USDC` for a pair, `#14730` for an outcome side.
#[must_use]
pub fn l2_book_request(coin: &str, precision: Precision) -> String {
    let number = |value: u32| Json::Number(value.to_string());
    let figures = precision.figures.map(|figures| ("nSigFigs", number(figures)));
    let step = precision.step.map(|step| ("mantissa", number(step)));
    let fields: Vec<(&str, Json)> =
        [Some(("coin", Json::String(coin.to_owned()))), figures, step].into_iter().flatten().collect();
    info_request("l2Book", &fields)
}

/// The book of `coin` at `precision`.
pub async fn l2_book(transport: &impl Transport, coin: &str, precision: Precision) -> Result<Book, ApiError> {
    info(transport, l2_book_request(coin, precision)).await
}

/// One price level: `n` orders resting at `px` for `sz` in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Level {
    /// The price.
    pub px: Dec,
    /// The size resting at the price, in the base asset.
    pub sz: Dec,
    /// Number of orders at this price, not an amount.
    pub n: u32,
}

/// `l2Book`: at most twenty levels a side, best first. There is no sequence number; each read
/// or frame replaces the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "Raw")]
pub struct Book {
    /// The coin, as asked for.
    pub coin: String,
    /// Milliseconds.
    pub time: u64,
    /// Highest first.
    pub bids: Vec<Level>,
    /// Lowest first.
    pub asks: Vec<Level>,
    /// Sent with a bucketed book (`nSigFigs`): the gap between the best levels.
    pub spread: Option<Dec>,
}

/// `levels` on the wire: bids, then asks.
#[derive(Deserialize)]
struct Raw {
    coin: String,
    time: u64,
    levels: (Vec<Level>, Vec<Level>),
    #[serde(default)]
    spread: Option<Dec>,
}

impl From<Raw> for Book {
    fn from(raw: Raw) -> Self {
        let (bids, asks) = raw.levels;
        Book { coin: raw.coin, time: raw.time, bids, asks, spread: raw.spread }
    }
}

impl Book {
    /// The best bid and the best ask.
    #[must_use]
    pub fn touch(&self) -> (Option<Dec>, Option<Dec>) {
        (self.bids.first().map(|level| level.px), self.asks.first().map(|level| level.px))
    }

    /// Exactly halfway between the best bid and ask, one place finer than either. `None` when a
    /// side is empty.
    #[must_use]
    pub fn mid(&self) -> Option<Dec> {
        let (Some(bid), Some(ask)) = self.touch() else { return None };
        halfway(bid, ask)
    }
}

/// Exactly halfway between two prices, one place finer than either.
fn halfway(bid: Dec, ask: Dec) -> Option<Dec> {
    let places = bid.places().max(ask.places()).saturating_add(1);
    bid.checked_add(ask)?.checked_div(Dec::from_int(2), places, Round::Down)
}

#[cfg(test)]
#[path = "tests/book.rs"]
mod tests;
