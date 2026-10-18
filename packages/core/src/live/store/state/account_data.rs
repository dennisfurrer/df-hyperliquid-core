//! Account data into the store: the mode, resting orders, fills and the ledger. A piece of an
//! account is kept only while an interest wants it.

use crate::account::history::{Fill, LedgerUpdate};
use crate::account::{Abstraction, FrontendOrder};
use crate::live::Subscription;
use crate::live::store::{Account, Folded, Store};
use crate::utils::Address;

impl Store {
    pub(super) fn fold_mode(&mut self, now: u64, user: Address, mode: Abstraction) -> Folded {
        self.with(user, &Subscription::WebData3 { user }, |account| account.set_mode(now, mode))
    }

    pub(super) fn fold_orders(&mut self, now: u64, user: Address, dex: &str, orders: Vec<FrontendOrder>) -> Folded {
        let wanted = Subscription::OpenOrders { user, dex: dex.to_owned() };
        self.with(user, &wanted, |account| account.set_orders(now, dex, orders))
    }

    pub(super) fn fold_fills(&mut self, now: u64, user: Address, fills: Vec<Fill>, whole: bool) -> Folded {
        let wanted = Subscription::UserFills { user, aggregate_by_time: false };
        self.with(user, &wanted, |account| account.fold_fills(now, fills, whole))
    }

    pub(super) fn fold_ledger(&mut self, now: u64, user: Address, rows: Vec<LedgerUpdate>, whole: bool) -> Folded {
        let wanted = Subscription::UserNonFundingLedgerUpdates { user };
        self.with(user, &wanted, |account| account.fold_ledger(now, rows, whole))
    }

    /// Changes `user`'s clearinghouse, while its state is wanted.
    pub(super) fn with_state(&mut self, user: Address, change: impl FnOnce(&mut Account) -> bool) -> Folded {
        self.with(user, &Subscription::AllDexsClearinghouseState { user }, change)
    }

    /// Changes `user`'s account while `wanted` is: what no interest asked for is not kept.
    pub(super) fn with(
        &mut self,
        user: Address,
        wanted: &Subscription,
        change: impl FnOnce(&mut Account) -> bool,
    ) -> Folded {
        if !self.interests.wants(wanted) {
            return Folded::Dropped;
        }
        Folded::of(change(self.accounts.entry(user).or_default()))
    }
}

#[cfg(test)]
#[path = "tests/account_data.rs"]
mod tests;
