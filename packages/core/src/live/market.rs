//! The websocket's own market frames: the best bid and offer, one dex's mids, and the `l2` book's
//! snapshot and diffs. Books, trades, candles and asset contexts come in the shapes the `/info`
//! reads answer, the [`crate::markets`] types. Every price and size is an exact [`Dec`] read from the venue's string; a number written
//! as a float is refused, never rounded.

use std::cmp::Ordering;

use crate::markets::{Book, Level};
use crate::utils::Dec;
use serde::Deserialize;

/// `bbo`: the best bid and offer, sent only when one of them moves. A side with nothing
/// resting is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawBbo")]
pub struct Bbo {
    /// The coin.
    pub coin: String,
    /// Milliseconds.
    pub time: u64,
    /// The best bid; `None` when no bid is resting.
    pub bid: Option<Level>,
    /// The best ask; `None` when no ask is resting.
    pub ask: Option<Level>,
}

#[derive(Deserialize)]
struct RawBbo {
    coin: String,
    time: u64,
    bbo: (Option<Level>, Option<Level>),
}

impl From<RawBbo> for Bbo {
    fn from(raw: RawBbo) -> Self {
        let (bid, ask) = raw.bbo;
        Bbo { coin: raw.coin, time: raw.time, bid, ask }
    }
}

/// `allMids`: every coin's mid on one perp dex, in the order sent. The first dex's (`dex` empty)
/// also carries spot pairs (`@107`, `PURR/USDC`) and outcome sides (`#14720`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Mids {
    /// The dex, `""` for the first perp dex and when not sent.
    #[serde(default)]
    pub dex: String,
    /// The map `allMids` answers.
    pub mids: crate::markets::mids::Mids,
}

impl Mids {
    /// A coin's mid, the coin spelled as the venue spells it.
    #[must_use]
    pub fn get(&self, coin: &str) -> Option<Dec> {
        self.mids.get(coin)
    }
}

/// One `l2` frame: the book's snapshot first, then each change as a diff. A diff comes either as
/// it is (`u`) or as base64 of raw DEFLATE (RFC 1951) over its JSON (`c`), kept as sent: inflating
/// it needs a decompressor this crate does not carry. The inflated text reads as an [`L2Diff`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum L2 {
    /// `s`: up to twenty levels a side, as `l2Book` sends them.
    #[serde(rename = "s")]
    Snapshot(Book),
    /// `u`: a diff as it is.
    #[serde(rename = "u")]
    Diff(L2Diff),
    /// `c`: a diff, base64 of raw DEFLATE over its JSON.
    #[serde(rename = "c")]
    Compressed(String),
}

/// One level an `l2` diff adds or changes: a price and the size now resting there. Diffs carry no
/// order count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct L2Level {
    /// `p`: the price.
    #[serde(rename = "p")]
    pub px: Dec,
    /// `s`: the size now resting at the price, in the coin's units.
    #[serde(rename = "s")]
    pub sz: Dec,
}

/// What changed in an `l2` book since the frame before. To apply it, the levels at the indexes in
/// `removed_bids` and `removed_asks` (positions in the book before) go first, then each level in
/// `bids` and `asks` is added or replaced by price; [`L2Diff::apply`] does both.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawL2Diff")]
pub struct L2Diff {
    /// `c`: the coin.
    pub coin: String,
    /// Milliseconds.
    pub time: u64,
    /// Bid levels added or changed.
    pub bids: Vec<L2Level>,
    /// Ask levels added or changed.
    pub asks: Vec<L2Level>,
    /// Positions of the bids removed, in the book before this diff.
    pub removed_bids: Vec<usize>,
    /// Positions of the asks removed, in the book before this diff.
    pub removed_asks: Vec<usize>,
    /// Sent with a bucketed book (`s` on the subscription), as `l2Book`'s `spread` is.
    pub spread: Option<Dec>,
}

/// `{"c","t","l":[bids,asks],"r":[bids,asks],"s"}` on the wire.
#[derive(Deserialize)]
struct RawL2Diff {
    c: String,
    t: u64,
    l: (Vec<L2Level>, Vec<L2Level>),
    r: (Vec<usize>, Vec<usize>),
    #[serde(default)]
    s: Option<Dec>,
}

impl From<RawL2Diff> for L2Diff {
    fn from(raw: RawL2Diff) -> Self {
        let ((bids, asks), (removed_bids, removed_asks)) = (raw.l, raw.r);
        L2Diff { coin: raw.c, time: raw.t, bids, asks, removed_bids, removed_asks, spread: raw.s }
    }
}

/// An `l2` diff that does not fit the book it was applied to.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDiff {
    /// The diff is for another coin than the book.
    #[error("a diff for {diff} applied to the book of {book}")]
    Coin {
        /// The book's coin.
        book: String,
        /// The diff's coin.
        diff: String,
    },
    /// The diff removes a level past the end of a side: a frame was missed.
    #[error("a diff removes level {index} of a side that has {levels}: a frame was missed")]
    Index {
        /// The position the diff removes.
        index: usize,
        /// The levels the side has.
        levels: usize,
    },
}

impl L2Diff {
    /// Applies the diff to `book`, the book as of the frame before: the removed levels go, then
    /// each level in the diff replaces the one at its price or is added in place, each side kept
    /// best first. A level the diff adds or changes has `n` 0, since diffs carry no order count.
    /// The time and spread become the diff's. A diff for another coin, or one that removes a level
    /// the book does not have, is refused and the book is left as it was.
    pub fn apply(&self, book: &mut Book) -> Result<(), BadDiff> {
        if book.coin != self.coin {
            return Err(BadDiff::Coin { book: book.coin.clone(), diff: self.coin.clone() });
        }
        let bids = changed(&book.bids, &self.removed_bids, &self.bids, |a, b| b.cmp(a))?;
        let asks = changed(&book.asks, &self.removed_asks, &self.asks, Ord::cmp)?;
        book.bids = bids;
        book.asks = asks;
        book.time = self.time;
        book.spread = self.spread;
        Ok(())
    }
}

/// One side after a diff, sorted best first by `best`.
fn changed(
    side: &[Level],
    removed: &[usize],
    levels: &[L2Level],
    best: fn(&Dec, &Dec) -> Ordering,
) -> Result<Vec<Level>, BadDiff> {
    if let Some(&index) = removed.iter().find(|&&index| index >= side.len()) {
        return Err(BadDiff::Index { index, levels: side.len() });
    }
    let mut kept: Vec<Level> =
        side.iter().enumerate().filter(|(index, _)| !removed.contains(index)).map(|(_, level)| *level).collect();
    for level in levels {
        let now = Level { px: level.px, sz: level.sz, n: 0 };
        match kept.binary_search_by(|resting| best(&resting.px, &level.px)) {
            Ok(at) => kept[at] = now,
            Err(at) => kept.insert(at, now),
        }
    }
    Ok(kept)
}

#[cfg(test)]
#[path = "tests/market.rs"]
mod tests;
