//! Whether a side's quote is a probability or only a number. A mid is honest on a tight, deep book
//! and a placeholder on a dead one, so [`assess`] reads the book first. The thresholds come from the
//! venue's observed spread and depth distribution.

use crate::markets::outcomes::book::{Book, Level};
use crate::markets::outcomes::px::{Mid, Px};
use crate::utils::Dec;

/// The widest spread at which a mid is a probability: two cents.
pub const WIDE_SPREAD: Px = cents(2);

/// Resting money is counted this far from the touch, each side: two cents.
pub const DEPTH_WINDOW: Px = cents(2);

/// The least resting notional within [`DEPTH_WINDOW`] for a firm quote, in the quote token.
pub const MIN_DEPTH: Dec = Dec::from_int(100);

/// A side resting this close to 0 or 1 is an absent side wearing a price.
const BOUNDARY: Px = cents(1);

const fn cents(count: u32) -> Px {
    match Px::from_ticks(count * 1_000) {
        Some(price) => price,
        None => Px::ZERO,
    }
}

/// What a side's quote is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Settled,
    /// A side missing, or parked within a cent of 0 or 1; `mid` is the midpoint a printed number
    /// would be, if both sides rest.
    Dead {
        mid: Option<Mid>,
    },
    /// Both sides real but wider or shallower than firm: a range, not a point.
    Thin {
        bid: Px,
        ask: Px,
        spread: Px,
        depth: Dec,
    },
    /// Both sides real, at most [`WIDE_SPREAD`] apart, with at least [`MIN_DEPTH`] within
    /// [`DEPTH_WINDOW`] of the touch: the mid is a probability.
    Firm {
        mid: Mid,
        spread: Px,
        depth: Dec,
    },
}

impl Quality {
    /// The probability to print, only for a firm quote.
    #[must_use]
    pub const fn probability(&self) -> Option<Mid> {
        match self {
            Quality::Firm { mid, .. } => Some(*mid),
            _ => None,
        }
    }

    /// The money behind the quote, only where there is a market.
    #[must_use]
    pub const fn depth(&self) -> Option<Dec> {
        match self {
            Quality::Firm { depth, .. } | Quality::Thin { depth, .. } => Some(*depth),
            _ => None,
        }
    }
}

/// What rests within `window` of the touch on both sides, size times price.
#[must_use]
pub fn depth_within(book: &Book, window: Px) -> Dec {
    let near = |levels: &[Level]| {
        let Some(touch) = levels.first().map(|level| level.px) else {
            return Dec::ZERO;
        };
        levels
            .iter()
            .filter(|level| level.px.abs_diff(touch) <= window.ticks())
            .map(|level| level.sz.notional(level.px))
            .fold(Dec::ZERO, |sum, worth| sum.checked_add(worth).unwrap_or(sum))
    };
    let (bids, asks) = (near(&book.bids), near(&book.asks));
    bids.checked_add(asks).unwrap_or(bids)
}

/// Reads the book. A settled side is [`Quality::Settled`] whatever rests.
#[must_use]
pub fn assess(book: &Book, settled: bool) -> Quality {
    if settled {
        return Quality::Settled;
    }
    let (bid, ask) = (book.bids.first().map(|level| level.px), book.asks.first().map(|level| level.px));
    let mid = bid.zip(ask).map(|(bid, ask)| midpoint(bid, ask));
    let (Some(bid), Some(ask)) = (bid, ask) else {
        return Quality::Dead { mid };
    };
    if bid <= BOUNDARY || ask >= BOUNDARY.complement() {
        return Quality::Dead { mid };
    }
    let spread = Px::from_ticks(ask.ticks().saturating_sub(bid.ticks())).unwrap_or(Px::ZERO);
    let depth = depth_within(book, DEPTH_WINDOW);
    match spread > WIDE_SPREAD || depth < MIN_DEPTH {
        true => Quality::Thin { bid, ask, spread, depth },
        false => Quality::Firm { mid: midpoint(bid, ask), spread, depth },
    }
}

/// Half way between two prices, exactly: a half tick is a mid's to keep.
const fn midpoint(bid: Px, ask: Px) -> Mid {
    match Mid::from_units((Mid::of(bid).units() + Mid::of(ask).units()) / 2) {
        Some(mid) => mid,
        None => Mid::ONE,
    }
}

#[cfg(test)]
#[path = "tests/quote.rs"]
mod tests;
