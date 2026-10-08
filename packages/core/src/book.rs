//! `l2Book`: a market's visible book, the same shape whether `/info` answers it or the socket
//! sends it. Every price and size is an exact [`Dec`] read from the venue's string.

use serde::Deserialize;

use crate::dec::{Dec, Round};

/// One price level: `n` orders resting at `px` for `sz` in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Level {
    pub px: Dec,
    pub sz: Dec,
    /// Orders at this price: a count, not an amount.
    pub n: u32,
}

/// `l2Book`: at most twenty levels a side, best first. There is no sequence number; each read
/// or frame replaces the last.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "Raw")]
pub struct Book {
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
    #[must_use]
    pub fn bids(&self) -> &[Level] {
        &self.bids
    }

    #[must_use]
    pub fn asks(&self) -> &[Level] {
        &self.asks
    }

    /// The best bid and the best ask.
    #[must_use]
    pub fn touch(&self) -> (Option<Dec>, Option<Dec>) {
        (self.bids.first().map(|level| level.px), self.asks.first().map(|level| level.px))
    }

    /// Halfway between the touch, exactly, a place finer than either price: what a market order
    /// is bounded from. `None` with a side empty.
    #[must_use]
    pub fn mid(&self) -> Option<Dec> {
        let (Some(bid), Some(ask)) = self.touch() else { return None };
        halfway(bid, ask)
    }
}

/// Exactly halfway between two prices, a place finer than either.
fn halfway(bid: Dec, ask: Dec) -> Option<Dec> {
    let places = bid.places().max(ask.places()).saturating_add(1);
    bid.checked_add(ask)?.checked_div(Dec::from_int(2), places, Round::Down)
}

#[cfg(test)]
#[path = "tests/book.rs"]
mod tests;
