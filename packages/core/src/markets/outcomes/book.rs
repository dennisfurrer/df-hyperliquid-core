//! One outcome side's book on the 0.00001 grid, from a [`crate::markets::Book`] (read with
//! [`crate::markets::book::l2_book`] for `#<encoding>`). A level off the grid, or a coin that is no
//! outcome side, refuses the whole book.

use crate::markets::outcomes::coin::Coin;
use crate::markets::outcomes::px::{BadPx, BadQty, Px, Qty};
use serde::Deserialize;

/// One price on the book: what rests there and in how many orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Level {
    /// The price, on the 0.00001 grid.
    pub px: Px,
    /// The whole contracts resting at the price.
    pub sz: Qty,
    /// How many orders rest at the price.
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
    /// The outcome side the book is for.
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
    /// The book's coin is no outcome side (`#<encoding>`); holds the coin as written.
    #[error("`{0}` is not an outcome side")]
    Coin(String),
    /// A level's price is off the grid.
    #[error(transparent)]
    Px(#[from] BadPx),
    /// A level's size is not whole contracts.
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
