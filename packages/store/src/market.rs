//! What the store holds of the markets: each wanted market's book, prints and candles, and,
//! for every market, its mid and its context; the perp dexes' universes and collateral, and spot
//! meta, which place contexts and price balances.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashSet},
};

use hl_account::Collateral;
use hl_core::Dec;
use hl_perps::PerpCtx;
use hl_spot::{SpotCtx, SpotMeta};
use hl_ws::{Book, Candle, Interval, Trade};

use crate::stamp::Stamped;

/// Prints kept a market, newest first (atlas keeps 200 for its readers).
pub const TAPE: usize = 200;

/// Candles kept a market and interval, oldest first: as many as `candleSnapshot` serves.
pub const BARS: usize = 5_000;

/// A market's context, by the kind of market it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context<'a> {
    Perp(&'a PerpCtx),
    /// A spot pair's or an outcome side's.
    Spot(&'a SpotCtx),
}

impl Context<'_> {
    #[must_use]
    pub const fn mark(&self) -> Dec {
        match self {
            Context::Perp(ctx) => ctx.mark_px,
            Context::Spot(ctx) => ctx.mark_px,
        }
    }

    /// `None` on a market with no book.
    #[must_use]
    pub const fn mid(&self) -> Option<Dec> {
        match self {
            Context::Perp(ctx) => ctx.mid_px,
            Context::Spot(ctx) => ctx.mid_px,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Markets {
    pub(crate) books: BTreeMap<String, Stamped<Book>>,
    pub(crate) tapes: BTreeMap<String, Stamped<Vec<Trade>>>,
    pub(crate) candles: BTreeMap<(String, Interval), Stamped<Vec<Candle>>>,
    /// Each perp dex's mids, `""` the first.
    pub(crate) mids: BTreeMap<String, Stamped<BTreeMap<String, Dec>>>,
    pub(crate) perps: BTreeMap<String, Stamped<PerpCtx>>,
    pub(crate) spots: BTreeMap<String, Stamped<SpotCtx>>,
    /// Each perp dex's coins in the order its contexts come.
    pub(crate) universes: BTreeMap<String, Vec<String>>,
    pub(crate) collateral: Collateral,
    pub(crate) spot_meta: Option<SpotMeta>,
}

/// A candle into candles kept oldest first (hip4-tui's `fold_bar`): the one forming is
/// replaced, the next goes on the end, a late one lands on its own start, and the oldest go past
/// [`BARS`]. Says whether anything changed.
pub(crate) fn fold_bar(bars: &mut Vec<Candle>, bar: Candle) -> bool {
    let slot = match bars.last_mut() {
        Some(last) if last.open_time > bar.open_time => bars.iter_mut().find(|one| one.open_time == bar.open_time),
        Some(last) if last.open_time == bar.open_time => Some(last),
        _ => {
            bars.push(bar);
            if bars.len() > BARS {
                let extra = bars.len() - BARS;
                bars.drain(..extra);
            }
            return true;
        }
    };
    match slot {
        Some(held) if *held != bar => {
            *held = bar;
            true
        }
        _ => false,
    }
}

/// Prints into a tape kept newest first: one already held (the same `tid` and time; the tape
/// is one coin's) is not added again, and the oldest go past [`TAPE`]. `prints` come newest
/// first. Says whether any was new.
pub(crate) fn fold_prints(tape: &mut Vec<Trade>, prints: impl IntoIterator<Item = Trade>) -> bool {
    let mut seen: HashSet<(u64, u64)> = tape.iter().map(|print| (print.tid, print.time)).collect();
    let mut fresh: Vec<Trade> = prints.into_iter().filter(|print| seen.insert((print.tid, print.time))).collect();
    if fresh.is_empty() {
        return false;
    }
    fresh.append(tape);
    fresh.sort_by_key(|print| Reverse(print.time));
    fresh.truncate(TAPE);
    *tape = fresh;
    true
}
