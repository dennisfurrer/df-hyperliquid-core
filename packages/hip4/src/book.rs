//! `l2Book` for one side of an outcome: its resting levels, best first. Every
//! level of an outcome book sits on the 0.00001 grid, so a level off it, or a
//! coin that is no outcome side, refuses the whole book rather than reading
//! as something it is not.

use hl_core::{ApiError, Json, Transport, info, info_request};
use serde::{Deserialize, Deserializer, de};

use crate::{
    coin::Coin,
    px::{Px, Qty},
};

/// `{"type":"l2Book","coin":"#14730"}`.
#[must_use]
pub fn book_request(coin: Coin) -> String {
    info_request("l2Book", &[("coin", Json::String(coin.to_string()))])
}

pub async fn book(transport: &impl Transport, coin: Coin) -> Result<Book, ApiError> {
    info(transport, book_request(coin)).await
}

/// One price on the book: what rests there and in how many orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Level {
    pub px: Px,
    pub sz: Qty,
    pub n: u32,
}

/// One side's book at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub coin: Coin,
    /// Milliseconds since 1970, the venue's own.
    pub time: u64,
    /// Highest first.
    pub bids: Vec<Level>,
    /// Lowest first.
    pub asks: Vec<Level>,
}

impl Book {
    /// The best bid and the best ask, when both sides rest.
    #[must_use]
    pub fn top(&self) -> Option<(Px, Px)> {
        Some((self.bids.first()?.px, self.asks.first()?.px))
    }
}

impl<'de> Deserialize<'de> for Book {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            coin: String,
            #[serde(default)]
            time: u64,
            levels: (Vec<Level>, Vec<Level>),
        }

        let raw = Raw::deserialize(deserializer)?;
        let coin = Coin::parse(&raw.coin)
            .ok_or_else(|| de::Error::custom(format!("`{}` is not an outcome side", raw.coin)))?;
        let (bids, asks) = raw.levels;
        Ok(Book { coin, time: raw.time, bids, asks })
    }
}

#[cfg(test)]
#[path = "tests/book.rs"]
mod tests;
