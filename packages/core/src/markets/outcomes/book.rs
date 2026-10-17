//! `l2Book` for one outcome side, read onto the 0.00001 grid. A level off the grid, or a coin that
//! is no outcome side, refuses the whole book.

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::outcomes::coin::Coin;
use crate::markets::outcomes::px::{BadPx, BadQty, Px, Qty};
use crate::utils::Json;
use serde::Deserialize;

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

impl TryFrom<crate::markets::book::Level> for Level {
    type Error = BadBook;

    fn try_from(level: crate::markets::book::Level) -> Result<Self, Self::Error> {
        Ok(Level { px: Px::from_dec(level.px)?, sz: Qty::from_dec(level.sz)?, n: level.n })
    }
}

/// One side's book at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "crate::markets::book::Book")]
pub struct Book {
    pub coin: Coin,
    /// Milliseconds.
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

/// Why a book is no outcome book.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadBook {
    #[error("`{0}` is not an outcome side")]
    Coin(String),
    #[error(transparent)]
    Px(#[from] BadPx),
    #[error(transparent)]
    Qty(#[from] BadQty),
}

/// The venue's book, onto the outcome grid.
impl TryFrom<crate::markets::book::Book> for Book {
    type Error = BadBook;

    fn try_from(book: crate::markets::book::Book) -> Result<Self, Self::Error> {
        let coin = Coin::parse(&book.coin).ok_or(BadBook::Coin(book.coin))?;
        let levels = |levels: Vec<crate::markets::book::Level>| -> Result<Vec<Level>, BadBook> {
            levels.into_iter().map(Level::try_from).collect()
        };
        Ok(Book { coin, time: book.time, bids: levels(book.bids)?, asks: levels(book.asks)? })
    }
}

#[cfg(test)]
#[path = "tests/book.rs"]
mod tests;
