//! Market data into the store: mids, books, prints, candles and contexts, and the metas that
//! place contexts and price balances. A book, prints or candles are kept only while an interest
//! wants them.

use std::collections::BTreeMap;

use crate::client::USDC_INDEX;
use crate::live::Subscription;
use crate::live::store::market::{fold_bar, fold_prints};
use crate::live::store::state::book;
use crate::live::store::{BARS, Folded, Stamped, Store};
use crate::markets::perps::PerpCtx;
use crate::markets::spot::{SpotCtx, SpotMeta};
use crate::markets::{Book, Candle, Interval, Mids, Trade};
use crate::utils::Dec;

impl Store {
    pub(super) fn fold_mids(&mut self, now: u64, dex: &str, mids: &Mids) -> Folded {
        let mids: BTreeMap<String, Dec> = mids.iter().map(|(coin, mid)| (coin.to_owned(), mid)).collect();
        match self.markets.mids.get_mut(dex) {
            Some(held) => Folded::of(held.set(now, mids)),
            None => {
                self.markets.mids.insert(dex.to_owned(), Stamped::new(now, mids));
                Folded::Changed
            }
        }
    }

    /// A book replaces the one held unless it is older by its own time. A grouped book (one
    /// with a spread) is not the full book the store asks for.
    pub(super) fn fold_book(&mut self, now: u64, sent: Book) -> Folded {
        if sent.spread.is_some() || !self.interests.wants(&book(&sent.coin)) {
            return Folded::Dropped;
        }
        match self.markets.books.get_mut(&sent.coin) {
            Some(held) if held.time > sent.time => Folded::Dropped,
            Some(held) => Folded::of(held.set(now, sent)),
            None => {
                self.markets.books.insert(sent.coin.clone(), Stamped::new(now, sent));
                Folded::Changed
            }
        }
    }

    /// Prints, newest first, onto each wanted coin's tape.
    pub(super) fn fold_prints(&mut self, now: u64, prints: impl IntoIterator<Item = Trade>) -> Folded {
        let mut by_coin: BTreeMap<String, Vec<Trade>> = BTreeMap::new();
        for print in prints {
            by_coin.entry(print.coin.clone()).or_default().push(print);
        }
        let mut folded = Folded::Dropped;
        for (coin, prints) in by_coin {
            if !self.interests.wants(&Subscription::Trades { coin: coin.clone() }) {
                continue;
            }
            let tape = self.markets.tapes.entry(coin).or_insert_with(|| Stamped::new(now, Vec::new()));
            let changed = fold_prints(&mut tape.value, prints);
            tape.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// Candles from the socket, each onto its wanted market and interval.
    pub(super) fn fold_candles(&mut self, now: u64, candles: impl IntoIterator<Item = Candle>) -> Folded {
        let mut folded = Folded::Dropped;
        for candle in candles {
            let key = (candle.coin.clone(), candle.interval);
            if !self.interests.wants(&Subscription::Candle { coin: key.0.clone(), interval: key.1 }) {
                continue;
            }
            let bars = self.markets.candles.entry(key).or_insert_with(|| Stamped::new(now, Vec::new()));
            let changed = fold_bar(&mut bars.value, candle);
            bars.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// Candles read, oldest first: the history goes under what the socket already sent, and
    /// the socket's win.
    pub(super) fn fold_history(&mut self, now: u64, history: Vec<Candle>) -> Folded {
        let mut by_market: BTreeMap<(String, Interval), Vec<Candle>> = BTreeMap::new();
        for candle in history {
            by_market.entry((candle.coin.clone(), candle.interval)).or_default().push(candle);
        }
        let mut folded = Folded::Dropped;
        for (key, mut history) in by_market {
            if !self.interests.wants(&Subscription::Candle { coin: key.0.clone(), interval: key.1 }) {
                continue;
            }
            let bars = self.markets.candles.entry(key).or_insert_with(|| Stamped::new(now, Vec::new()));
            history.sort_by_key(|candle| candle.open_time);
            let live = std::mem::take(&mut bars.value);
            let mut merged = history;
            for bar in live.iter().cloned() {
                let _ = fold_bar(&mut merged, bar);
            }
            if merged.len() > BARS {
                let extra = merged.len() - BARS;
                merged.drain(..extra);
            }
            let changed = merged != live;
            bars.value = merged;
            bars.touch(now, changed);
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    pub(super) fn fold_perp_ctxs(&mut self, now: u64, ctxs: impl IntoIterator<Item = (String, PerpCtx)>) -> Folded {
        let mut folded = Folded::Dropped;
        for (coin, ctx) in ctxs {
            let changed = match self.markets.perps.get_mut(&coin) {
                Some(held) => held.set(now, ctx),
                None => {
                    self.markets.perps.insert(coin, Stamped::new(now, ctx));
                    true
                }
            };
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    pub(super) fn fold_spot_ctxs(&mut self, now: u64, ctxs: impl IntoIterator<Item = (String, SpotCtx)>) -> Folded {
        let mut folded = Folded::Dropped;
        for (coin, ctx) in ctxs {
            let changed = match self.markets.spots.get_mut(&coin) {
                Some(held) => held.set(now, ctx),
                None => {
                    self.markets.spots.insert(coin, Stamped::new(now, ctx));
                    true
                }
            };
            folded = folded.or(Folded::of(changed));
        }
        folded
    }

    /// One dex's contexts in its universe's order. With no universe read for the dex there is
    /// nothing to place them on; past its end (a perp listed since) they wait for the next read.
    pub(super) fn fold_dex_ctxs(&mut self, now: u64, dex: &str, ctxs: impl IntoIterator<Item = PerpCtx>) -> Folded {
        let Some(coins) = self.markets.universes.get(dex) else { return Folded::Dropped };
        let placed: Vec<(String, PerpCtx)> = coins.iter().cloned().zip(ctxs).collect();
        self.fold_perp_ctxs(now, placed)
    }

    /// A dex's universe and collateral, from its meta.
    pub(super) fn list(&mut self, dex: &str, coins: Vec<String>, collateral: Option<u32>) -> Folded {
        let token = collateral.unwrap_or(USDC_INDEX);
        let changed = self.markets.universes.get(dex) != Some(&coins) || self.markets.collateral.of(dex) != token;
        self.markets.universes.insert(dex.to_owned(), coins);
        self.markets.collateral = std::mem::take(&mut self.markets.collateral).with(dex, token);
        Folded::of(changed)
    }

    pub(super) fn fold_spot_meta(&mut self, meta: &SpotMeta) -> Folded {
        let changed = self.markets.spot_meta.as_ref() != Some(meta);
        self.markets.spot_meta = Some(meta.clone());
        Folded::of(changed)
    }
}

#[cfg(test)]
#[path = "tests/market_data.rs"]
mod tests;
