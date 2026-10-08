//! What the store holds of one account, read at its own address: a subaccount is an account
//! of its own, with its own mode.
//!
//! - Its clearinghouse on each dex, its spot balances and its mode, and from them hl-account's
//!   [`Standing`], which usable balances and worth are read from.
//! - Its resting orders on each dex.
//! - Its fills, fundings and ledger, newest first. A snapshot (`isSnapshot`, or a read of the
//!   latest) replaces what is held, as hip4-tui does; anything else is added to it, and a row
//!   already held is not added twice.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashSet},
    hash::Hash,
};

use hl_account::{Abstraction, DexState, SpotState, Standing};
use hl_perps::{ClearinghouseState, Position};
use hl_trade::FrontendOrder;
use hl_ws::{Fill, Funding, LedgerUpdate};

use crate::stamp::{Stamped, put};

/// Rows of each history kept, newest first (hip4-tui keeps 4000 fills); the whole of it is a
/// read away.
pub const KEEP: usize = 4_000;

/// One account as the venue last told it.
#[derive(Debug, Clone, Default)]
pub struct Account {
    pub(crate) dexes: BTreeMap<String, Stamped<ClearinghouseState>>,
    pub(crate) spot: Option<Stamped<SpotState>>,
    pub(crate) mode: Option<Stamped<Abstraction>>,
    pub(crate) orders: BTreeMap<String, Stamped<Vec<FrontendOrder>>>,
    pub(crate) fills: Option<Stamped<Vec<Fill>>>,
    pub(crate) fundings: Option<Stamped<Vec<Funding>>>,
    pub(crate) ledger: Option<Stamped<Vec<LedgerUpdate>>>,
    /// hl-account's view of the dexes, the spot balances and the mode, kept in step with them.
    standing: Standing,
}

impl Account {
    /// The mode, the dexes and spot as hl-account reads an account.
    #[must_use]
    pub const fn standing(&self) -> &Standing {
        &self.standing
    }

    /// `None` until the socket or a read says.
    #[must_use]
    pub const fn mode(&self) -> Option<&Stamped<Abstraction>> {
        self.mode.as_ref()
    }

    #[must_use]
    pub const fn spot(&self) -> Option<&Stamped<SpotState>> {
        self.spot.as_ref()
    }

    /// One dex's clearinghouse, `""` the first.
    #[must_use]
    pub fn dex(&self, dex: &str) -> Option<&Stamped<ClearinghouseState>> {
        self.dexes.get(dex)
    }

    /// Every dex told of, the first first.
    pub fn dexes(&self) -> impl Iterator<Item = (&str, &Stamped<ClearinghouseState>)> {
        self.dexes.iter().map(|(dex, state)| (dex.as_str(), state))
    }

    /// Every open position, dex by dex.
    pub fn positions(&self) -> impl Iterator<Item = &Position> {
        self.dexes.values().flat_map(|state| state.positions())
    }

    /// The resting orders on one dex.
    #[must_use]
    pub fn orders(&self, dex: &str) -> Option<&Stamped<Vec<FrontendOrder>>> {
        self.orders.get(dex)
    }

    /// Every resting order, dex by dex, each dex's in the venue's order.
    pub fn all_orders(&self) -> impl Iterator<Item = &FrontendOrder> {
        self.orders.values().flat_map(|orders| orders.iter())
    }

    /// Newest first.
    #[must_use]
    pub const fn fills(&self) -> Option<&Stamped<Vec<Fill>>> {
        self.fills.as_ref()
    }

    /// Newest first.
    #[must_use]
    pub const fn fundings(&self) -> Option<&Stamped<Vec<Funding>>> {
        self.fundings.as_ref()
    }

    /// Newest first.
    #[must_use]
    pub const fn ledger(&self) -> Option<&Stamped<Vec<LedgerUpdate>>> {
        self.ledger.as_ref()
    }

    /// When anything about the account was last told.
    #[must_use]
    pub fn heard(&self) -> Option<u64> {
        let rows = [
            self.fills.as_ref().map(|fills| fills.heard),
            self.fundings.as_ref().map(|fundings| fundings.heard),
            self.ledger.as_ref().map(|ledger| ledger.heard),
        ];
        let orders = self.orders.values().map(|orders| Some(orders.heard));
        rows.into_iter().chain(orders).chain([self.state_heard()]).flatten().max()
    }

    /// When the dexes, the spot balances or the mode were last told.
    fn state_heard(&self) -> Option<u64> {
        let dexes = self.dexes.values().map(|state| state.heard);
        let rest = [self.spot.as_ref().map(|spot| spot.heard), self.mode.as_ref().map(|mode| mode.heard)];
        dexes.chain(rest.into_iter().flatten()).max()
    }

    /// One dex's clearinghouse, told at `now`.
    pub(crate) fn set_dex(&mut self, now: u64, dex: &str, state: ClearinghouseState) -> bool {
        let changed = match self.dexes.get_mut(dex) {
            Some(held) => held.set(now, state),
            None => {
                self.dexes.insert(dex.to_owned(), Stamped::new(now, state));
                true
            }
        };
        self.state_told(changed)
    }

    /// Every dex's clearinghouse, told at `now`: a dex not listed is gone.
    pub(crate) fn set_dexes(&mut self, now: u64, states: Vec<(String, ClearinghouseState)>) -> bool {
        let before = self.dexes.len();
        self.dexes.retain(|dex, _| states.iter().any(|(listed, _)| listed == dex));
        let mut changed = self.dexes.len() != before;
        for (dex, state) in states {
            changed |= match self.dexes.get_mut(&dex) {
                Some(held) => held.set(now, state),
                None => {
                    self.dexes.insert(dex, Stamped::new(now, state));
                    true
                }
            };
        }
        self.state_told(changed)
    }

    pub(crate) fn set_spot(&mut self, now: u64, spot: SpotState) -> bool {
        let changed = put(&mut self.spot, now, spot);
        self.state_told(changed)
    }

    pub(crate) fn set_mode(&mut self, now: u64, mode: Abstraction) -> bool {
        let changed = put(&mut self.mode, now, mode);
        self.state_told(changed)
    }

    pub(crate) fn set_orders(&mut self, now: u64, dex: &str, orders: Vec<FrontendOrder>) -> bool {
        match self.orders.get_mut(dex) {
            Some(held) => held.set(now, orders),
            None => {
                self.orders.insert(dex.to_owned(), Stamped::new(now, orders));
                true
            }
        }
    }

    /// Fills told at `now`, newest first; `whole` replaces what is held. A fill is the same fill
    /// by its `tid` and `oid`: the two sides of one trade share a `tid` (and can both be this
    /// account's), every fill of one transaction shares a hash, and a TWAP slice's hash is all
    /// zeros.
    pub(crate) fn fold_fills(&mut self, now: u64, fills: Vec<Fill>, whole: bool) -> bool {
        fold_rows(&mut self.fills, now, fills, whole, |fill| (fill.tid, fill.oid), |fill| fill.time)
    }

    /// Fundings told at `now`, newest first: one a position an hour, so the same by time and
    /// coin.
    pub(crate) fn fold_fundings(&mut self, now: u64, fundings: Vec<Funding>, whole: bool) -> bool {
        fold_rows(&mut self.fundings, now, fundings, whole, |row| (row.time, row.coin.clone()), |row| row.time)
    }

    /// Ledger rows told at `now`, newest first: the same only when every key and digit is.
    pub(crate) fn fold_ledger(&mut self, now: u64, rows: Vec<LedgerUpdate>, whole: bool) -> bool {
        fold_rows(
            &mut self.ledger,
            now,
            rows,
            whole,
            |row| (row.time, row.hash.clone(), row.delta.compact()),
            |row| row.time,
        )
    }

    /// Lets go of the pieces nothing wants any more: the state (dexes, spot and mode together),
    /// a dex's orders, the fills, the fundings, the ledger.
    pub(crate) fn keep(&mut self, keep: &Keep<'_>) {
        if !keep.state && (self.spot.is_some() || self.mode.is_some() || !self.dexes.is_empty()) {
            self.dexes.clear();
            self.spot = None;
            self.mode = None;
            self.restand();
        }
        self.orders.retain(|dex, _| (keep.orders)(dex));
        if !keep.fills {
            self.fills = None;
        }
        if !keep.fundings {
            self.fundings = None;
        }
        if !keep.ledger {
            self.ledger = None;
        }
    }

    /// Keeps hl-account's view in step: rebuilt when the state changed, its time moved when it
    /// was only told again. Gives back `changed`.
    fn state_told(&mut self, changed: bool) -> bool {
        match changed {
            true => self.restand(),
            false => self.standing.heard = self.state_heard(),
        }
        changed
    }

    fn restand(&mut self) {
        self.standing = Standing {
            mode: self.mode.as_ref().map(|mode| mode.value.clone()),
            dexes: self
                .dexes
                .iter()
                .map(|(dex, state)| DexState { dex: dex.clone(), state: crate::convert::perp_state(state) })
                .collect(),
            spot: self.spot.as_ref().map(|spot| spot.value.clone()),
            heard: self.state_heard(),
        };
    }
}

/// Rows told at `now`, newest first, into a history kept newest first and at most [`KEEP`]
/// long. `whole` replaces what is held; otherwise a row whose `key` is held already is not added
/// again, and among rows of one time the new go before the held. Says whether anything changed.
fn fold_rows<T: PartialEq, K: Hash + Eq>(
    slot: &mut Option<Stamped<Vec<T>>>,
    now: u64,
    rows: Vec<T>,
    whole: bool,
    key: impl Fn(&T) -> K,
    time: impl Fn(&T) -> u64,
) -> bool {
    let mut seen: HashSet<K> = HashSet::new();
    let held = match (whole, slot.as_mut()) {
        (false, Some(held)) => {
            seen.extend(held.iter().map(&key));
            Some(held)
        }
        _ => None,
    };
    let mut fresh: Vec<T> = rows.into_iter().filter(|row| seen.insert(key(row))).collect();
    let Some(held) = held else {
        fresh.sort_by_key(|row| Reverse(time(row)));
        fresh.truncate(KEEP);
        return put(slot, now, fresh);
    };
    if fresh.is_empty() {
        held.touch(now, false);
        return false;
    }
    fresh.append(&mut held.value);
    fresh.sort_by_key(|row| Reverse(time(row)));
    fresh.truncate(KEEP);
    held.value = fresh;
    held.touch(now, true);
    true
}

/// Which pieces of an account something still wants.
#[derive(Clone, Copy)]
pub(crate) struct Keep<'a> {
    pub(crate) state: bool,
    pub(crate) orders: &'a dyn Fn(&str) -> bool,
    pub(crate) fills: bool,
    pub(crate) fundings: bool,
    pub(crate) ledger: bool,
}
