//! An account on the outcome books: the sides it holds, read from `spotClearinghouseState`, the
//! authoritative answer, and its outcome fills and resting orders read from the rows of every
//! market. A structural fill (a split, merge, negation or settlement) is never volume.

use std::{convert::Infallible, fmt, str::FromStr};

use crate::account::history::{fills, orders};
use crate::account::holdings::spot::SpotState;
use crate::markets::outcomes::coin::Coin;
use crate::markets::outcomes::px::{Px, Qty, SignedQty};
use crate::trade::{Cloid, Side};
use crate::utils::{Dec, Round};

/// What a fill was, by its `dir`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// A buy that crossed a book (`Buy`).
    Buy,
    /// A sell that crossed a book (`Sell`).
    Sell,
    /// The quote token split into as many of each side of an outcome (`Split Outcome`).
    SplitOutcome,
    /// Both sides of an outcome merged back into the quote token (`Merge Outcome`).
    MergeOutcome,
    /// No shares of one outcome turned into Yes shares of every other outcome of its question
    /// (`Negate Outcome`).
    NegateOutcome,
    /// Yes shares of each outcome of a question merged back into the quote token
    /// (`Merge Question`).
    MergeQuestion,
    /// Paid out when the outcome settled (`Settlement`).
    Settlement,
    /// Anything else, kept as written.
    Other(String),
}

impl Direction {
    /// From the venue's `dir`: `Split Outcome`. Anything else is kept as [`Direction::Other`].
    #[must_use]
    pub fn read(text: &str) -> Direction {
        match text.trim() {
            "Buy" => Direction::Buy,
            "Sell" => Direction::Sell,
            "Split Outcome" => Direction::SplitOutcome,
            "Merge Outcome" => Direction::MergeOutcome,
            "Negate Outcome" => Direction::NegateOutcome,
            "Merge Question" => Direction::MergeQuestion,
            "Settlement" => Direction::Settlement,
            other => Direction::Other(other.to_owned()),
        }
    }

    /// As a lowercase slug: `split_outcome`, or an unknown `dir` as the venue wrote it.
    #[must_use]
    pub fn slug(&self) -> &str {
        match self {
            Direction::Buy => "buy",
            Direction::Sell => "sell",
            Direction::SplitOutcome => "split_outcome",
            Direction::MergeOutcome => "merge_outcome",
            Direction::NegateOutcome => "negate_outcome",
            Direction::MergeQuestion => "merge_question",
            Direction::Settlement => "settlement",
            Direction::Other(raw) => raw,
        }
    }

    /// Crossed a book: traded volume.
    #[must_use]
    pub const fn is_trade(&self) -> bool {
        matches!(self, Direction::Buy | Direction::Sell)
    }

    /// A complete set minted or burned against the protocol: never volume.
    #[must_use]
    pub const fn is_structural(&self) -> bool {
        matches!(
            self,
            Direction::SplitOutcome | Direction::MergeOutcome | Direction::NegateOutcome | Direction::MergeQuestion
        )
    }

    /// Paid out at settlement: never volume.
    #[must_use]
    pub const fn is_settlement(&self) -> bool {
        matches!(self, Direction::Settlement)
    }
}

impl FromStr for Direction {
    type Err = Infallible;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Direction::read(text))
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// As [`Direction::slug`] writes it: `split_outcome`, or an unknown `dir` as the venue wrote it.
impl serde::Serialize for Direction {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}

/// One outcome fill, read from a `userFills` or `userFillsByTime` row. Perp and spot rows come in
/// the same answer; [`Fill::read`] skips them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    /// The outcome side.
    pub coin: Coin,
    /// The account's side.
    pub side: Side,
    /// What the fill was, from its `dir`.
    pub direction: Direction,
    /// As reported: a structural fill's `1/n` is finer than the grid.
    pub price: Dec,
    /// The size filled, in contracts.
    pub size: Qty,
    /// When it filled (`time`), in milliseconds.
    pub at_ms: u64,
    /// The account's position in the side before this fill.
    pub start_position: SignedQty,
    /// After this fill.
    pub position: SignedQty,
    /// The profit or loss the fill realised (`closedPnl`), in the quote token.
    pub closed_pnl: Dec,
    /// Everything billed, the builder's fee included.
    pub fee: Dec,
    /// The token `fee` is paid in (`feeToken`).
    pub fee_token: String,
    /// What the order's builder took; `None` when the venue leaves it out.
    pub builder_fee: Option<Dec>,
    /// True for an order that took, false for a resting one that was taken.
    pub crossed: bool,
    /// The venue's order id (`oid`).
    pub oid: u64,
    /// The trade id (`tid`).
    pub tid: u64,
    /// The L1 transaction hash.
    pub hash: String,
    /// The order's client id as written; `None` when the order had none.
    pub cloid: Option<String>,
}

impl Fill {
    /// `None` for a row that is no outcome fill, or does not read.
    #[must_use]
    pub fn read(raw: &fills::Fill) -> Option<Fill> {
        if !raw.coin.starts_with('#') {
            return None;
        }
        let coin = Coin::parse(&raw.coin)?;
        let size = Qty::from_dec(raw.sz).ok()?;
        let start_position = SignedQty::from_dec(raw.start_position).ok()?;
        let moved = match raw.side {
            Side::Buy => SignedQty::long(size),
            Side::Sell => SignedQty::short(size),
        };
        Some(Fill {
            coin,
            side: raw.side,
            direction: Direction::read(&raw.dir),
            price: raw.px,
            size,
            at_ms: raw.time,
            start_position,
            position: start_position.saturating_add(moved),
            closed_pnl: raw.closed_pnl,
            fee: raw.fee,
            fee_token: raw.fee_token.clone(),
            builder_fee: raw.builder_fee,
            crossed: raw.crossed,
            oid: raw.oid,
            tid: raw.tid,
            hash: raw.hash.clone(),
            cloid: raw.cloid.clone(),
        })
    }

    /// Size times price, in the quote token.
    #[must_use]
    pub fn notional(&self) -> Dec {
        self.size.to_dec().checked_mul(self.price).unwrap_or(Dec::ZERO)
    }

    /// What the venue billed, the builder's fee taken out.
    #[must_use]
    pub fn venue_fee(&self) -> Option<Dec> {
        self.fee.checked_sub(self.builder_fee.unwrap_or(Dec::ZERO))
    }
}

/// An outcome order resting on the book, read from an `openOrders` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrder {
    /// The outcome side.
    pub coin: Coin,
    /// The side it rests on.
    pub side: Side,
    /// The limit price.
    pub price: Px,
    /// What still rests.
    pub size: Qty,
    /// The size when placed (`origSz`).
    pub original_size: Qty,
    /// When it was placed (`timestamp`), in milliseconds.
    pub placed_ms: u64,
    /// The venue's order id (`oid`).
    pub oid: u64,
    /// The client's order id; `None` when the order was sent without one.
    pub cloid: Option<Cloid>,
}

impl OpenOrder {
    /// `None` for a row that is no outcome order, or does not read.
    #[must_use]
    pub fn read(raw: &orders::OpenOrder) -> Option<OpenOrder> {
        if !raw.coin.starts_with('#') {
            return None;
        }
        Some(OpenOrder {
            coin: Coin::parse(&raw.coin)?,
            side: raw.side,
            price: Px::from_dec(raw.limit_px).ok()?,
            size: Qty::from_dec(raw.sz).ok()?,
            original_size: Qty::from_dec(raw.orig_sz).ok()?,
            placed_ms: raw.timestamp,
            oid: raw.oid,
            cloid: raw.cloid,
        })
    }

    /// What the book has already taken.
    #[must_use]
    pub const fn filled(&self) -> Qty {
        self.original_size.saturating_sub(self.size)
    }
}

/// One outcome side held, as the spot side reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    /// The outcome side.
    pub coin: Coin,
    /// Everything held, `hold` included.
    pub total: Dec,
    /// What resting sells hold.
    pub hold: Dec,
    /// What it cost.
    pub entry_ntl: Dec,
}

impl Holding {
    /// What no order holds.
    #[must_use]
    pub fn free(&self) -> Dec {
        self.total.checked_sub(self.hold).unwrap_or(Dec::ZERO).max(Dec::ZERO)
    }

    /// What it cost a contract.
    #[must_use]
    pub fn average(&self) -> Option<Dec> {
        match self.total.is_positive() {
            true => self.entry_ntl.checked_div(self.total, 8, Round::Nearest),
            false => None,
        }
    }
}

/// The outcome sides the account holds, in the venue's order. A row with
/// nothing in it, and every row that is no outcome side, is left out.
#[must_use]
pub fn holdings(state: &SpotState) -> Vec<Holding> {
    state
        .balances
        .iter()
        .filter(|row| !(row.total.is_zero() && row.hold.is_zero()))
        .filter_map(|row| {
            Some(Holding { coin: Coin::parse(&row.coin)?, total: row.total, hold: row.hold, entry_ntl: row.entry_ntl })
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/outcomes.rs"]
mod tests;
