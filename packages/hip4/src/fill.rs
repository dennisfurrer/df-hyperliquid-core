//! An account on the outcome books: what it holds, how it got there, and what
//! rests.
//!
//! Holdings come from the spot side (D9): `clearinghouseState` reports
//! nothing about outcomes at any `dex`, while `spotClearinghouseState` lists
//! every outcome side under a `+` spelling, in full. The balance is the
//! answer. Fills corroborate it and carry what a balance cannot (realized
//! profit, fees); where the two differ the fill window does not reach that
//! side's last trade, so the row is marked, not reconciled. `hold` is part of
//! `total` and never added to it.
//!
//! Structural fills are classified, never counted as volume (D10): splits,
//! merges, negations and settlements price at `1/n` or pay out at one and
//! zero, about a fifth of all outcome fills.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fmt,
    str::FromStr,
};

use hl_core::{Dec, Round, order::Cloid};
use hl_trade::Side;
use serde::Deserialize;

use crate::{
    coin::Coin,
    px::{Px, Qty, SignedQty},
};

/// What a fill was, by its `dir`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    Buy,
    Sell,
    SplitOutcome,
    MergeOutcome,
    NegateOutcome,
    MergeQuestion,
    Settlement,
    /// Anything else, kept as written.
    Other(String),
}

impl Direction {
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

/// One row of `userFills` or `userFillsByTime`, as written: hl-core's. Perp
/// and spot rows come in the same answer; [`Fill::read`] skips them.
pub type RawFill = hl_core::fill::Fill;

/// One outcome fill, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    pub coin: Coin,
    pub side: Side,
    pub direction: Direction,
    /// As reported: a structural fill's `1/n` is finer than the grid.
    pub price: Dec,
    pub size: Qty,
    pub at_ms: u64,
    pub start_position: SignedQty,
    /// After this fill.
    pub position: SignedQty,
    pub closed_pnl: Dec,
    /// Everything billed, the builder's fee included.
    pub fee: Dec,
    pub fee_token: String,
    pub builder_fee: Option<Dec>,
    /// True for an order that took, false for a resting one that was taken.
    pub crossed: bool,
    pub oid: u64,
    pub tid: u64,
    pub hash: String,
    pub cloid: Option<String>,
}

impl Fill {
    /// `None` for a row that is no outcome fill, or does not read.
    #[must_use]
    pub fn read(raw: &RawFill) -> Option<Fill> {
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

/// One row of `openOrders` or `frontendOpenOrders`, as written: hl-core's.
pub type RawOrder = hl_core::order::OpenOrder;

/// An outcome order resting on the book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrder {
    pub coin: Coin,
    pub side: Side,
    pub price: Px,
    /// What still rests.
    pub size: Qty,
    pub original_size: Qty,
    pub placed_ms: u64,
    pub oid: u64,
    pub cloid: Option<Cloid>,
}

impl OpenOrder {
    /// `None` for a row that is no outcome order, or does not read.
    #[must_use]
    pub fn read(raw: &RawOrder) -> Option<OpenOrder> {
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

/// What fills say is held in one side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub coin: Coin,
    /// From the newest fill alone.
    pub size: SignedQty,
    /// When that fill was.
    pub fixed_at_ms: u64,
    pub realized_pnl: Dec,
    pub fees: Dec,
    pub fills: usize,
}

/// The positions fills imply, largest first. Fills sharing a millisecond keep
/// the order they came in, newest first, as the venue sends them.
#[must_use]
pub fn positions(fills: &[Fill]) -> Vec<Position> {
    let mut newest_first: Vec<&Fill> = fills.iter().collect();
    newest_first.sort_by_key(|fill| std::cmp::Reverse(fill.at_ms));
    let mut found: BTreeMap<Coin, Position> = BTreeMap::new();
    for fill in newest_first {
        let position = found.entry(fill.coin).or_insert_with(|| Position {
            coin: fill.coin,
            size: fill.position,
            fixed_at_ms: fill.at_ms,
            realized_pnl: Dec::ZERO,
            fees: Dec::ZERO,
            fills: 0,
        });
        position.realized_pnl = position.realized_pnl.checked_add(fill.closed_pnl).unwrap_or(position.realized_pnl);
        position.fees = position.fees.checked_add(fill.fee).unwrap_or(position.fees);
        position.fills += 1;
    }
    let mut positions: Vec<Position> = found.into_values().collect();
    positions.sort_by(|a, b| b.size.abs().cmp(&a.size.abs()).then_with(|| a.coin.cmp(&b.coin)));
    positions
}

/// A window of fills summed, structural and settled notional kept apart from
/// what was traded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub fills: usize,
    /// Outcomes touched.
    pub markets: usize,
    pub trades: usize,
    pub traded_volume: Dec,
    pub structural_volume: Dec,
    pub settled_volume: Dec,
    pub realized_pnl: Dec,
    pub fees: Dec,
    /// Trades that rested and were taken.
    pub made: usize,
    /// Trades that took.
    pub taken: usize,
    pub by_direction: BTreeMap<String, usize>,
    pub first_ms: Option<u64>,
    pub last_ms: Option<u64>,
}

#[must_use]
pub fn summarize(fills: &[Fill]) -> Summary {
    let add = |sum: Dec, more: Dec| sum.checked_add(more).unwrap_or(sum);
    let mut summary = Summary {
        fills: fills.len(),
        markets: 0,
        trades: 0,
        traded_volume: Dec::ZERO,
        structural_volume: Dec::ZERO,
        settled_volume: Dec::ZERO,
        realized_pnl: Dec::ZERO,
        fees: Dec::ZERO,
        made: 0,
        taken: 0,
        by_direction: BTreeMap::new(),
        first_ms: None,
        last_ms: None,
    };
    let mut outcomes = BTreeSet::new();
    for fill in fills {
        outcomes.insert(fill.coin.outcome());
        summary.realized_pnl = add(summary.realized_pnl, fill.closed_pnl);
        summary.fees = add(summary.fees, fill.fee);
        if fill.direction.is_trade() {
            summary.trades += 1;
            summary.traded_volume = add(summary.traded_volume, fill.notional());
            match fill.crossed {
                true => summary.taken += 1,
                false => summary.made += 1,
            }
        } else if fill.direction.is_structural() {
            summary.structural_volume = add(summary.structural_volume, fill.notional());
        } else if fill.direction.is_settlement() {
            summary.settled_volume = add(summary.settled_volume, fill.notional());
        }
        *summary.by_direction.entry(fill.direction.slug().to_owned()).or_default() += 1;
        summary.first_ms = Some(summary.first_ms.map_or(fill.at_ms, |first| first.min(fill.at_ms)));
        summary.last_ms = Some(summary.last_ms.map_or(fill.at_ms, |last| last.max(fill.at_ms)));
    }
    summary.markets = outcomes.len();
    summary
}

/// `spotClearinghouseState`, the rows an outcome holding is read from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct SpotBalances {
    #[serde(default)]
    pub balances: Vec<SpotRow>,
}

/// One spot row as written. Outcome rows carry no `token`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotRow {
    pub coin: String,
    #[serde(default)]
    pub total: Dec,
    #[serde(default)]
    pub hold: Dec,
    #[serde(default)]
    pub entry_ntl: Dec,
}

/// One outcome side held, as the spot side reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
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
pub fn holdings(state: &SpotBalances) -> Vec<Holding> {
    state
        .balances
        .iter()
        .filter(|row| !(row.total.is_zero() && row.hold.is_zero()))
        .filter_map(|row| {
            Some(Holding { coin: Coin::parse(&row.coin)?, total: row.total, hold: row.hold, entry_ntl: row.entry_ntl })
        })
        .collect()
}

/// What the fills say of a holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Agrees,
    /// The fill window does not reach the side's last trade; the balance
    /// stands.
    Disagrees {
        from_fills: SignedQty,
    },
    /// No fill in the window touches it.
    Unseen,
}

/// Each holding beside what fills derive for it.
#[must_use]
pub fn corroborate<'a>(holdings: &'a [Holding], positions: &[Position]) -> Vec<(&'a Holding, Check)> {
    holdings
        .iter()
        .map(|holding| {
            let check = match positions.iter().find(|position| position.coin == holding.coin) {
                None => Check::Unseen,
                Some(position) if !position.size.is_short() && position.size.abs().to_dec() == holding.total => {
                    Check::Agrees
                }
                Some(position) => Check::Disagrees { from_fills: position.size },
            };
            (holding, check)
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/fill.rs"]
mod tests;
