//! The store's interests and the session they feed: what is wanted and followed, what changed
//! since the socket was last told, and [`Store::sync`], which refuses a session on another network.

use crate::live::store::account::Keep;
use crate::live::store::state::book;
use crate::live::store::{Changes, Handle, Interest, OtherNetwork, Store, Synced};
use crate::live::{Refused, Session, Subscription};
use crate::utils::Address;

impl Store {
    /// Needs `interest` kept live until the handle is dropped or released. Refused past the
    /// venue's limits, or when the venue would not take a subscription it needs.
    pub fn register(&mut self, interest: Interest) -> Result<Handle, Refused> {
        self.settle();
        self.interests.register(interest)
    }

    /// Ends the interest `handle` holds, as dropping it does, and lets go at once of what
    /// nothing needs any more.
    pub fn release(&mut self, handle: Handle) {
        drop(handle);
        self.settle();
    }

    /// Every subscription wanted now, in the order first wanted.
    #[must_use]
    pub fn wanted(&self) -> &[Subscription] {
        self.interests.wanted()
    }

    /// The accounts followed, in the order first followed; at most the limits' users.
    #[must_use]
    pub fn followed(&self) -> &[Address] {
        self.interests.users()
    }

    /// What to tell the socket since it was last told: unsubscribe what nothing needs, then
    /// subscribe what is newly needed.
    pub fn changes(&mut self) -> Changes {
        self.settle();
        self.interests.changes()
    }

    /// [`Store::changes`] into `session`: the frames to send now. A session on another network
    /// is refused before anything changes.
    pub fn sync(&mut self, session: &mut Session) -> Result<Synced, OtherNetwork> {
        if session.network() != self.network {
            return Err(OtherNetwork { store: self.network, session: session.network() });
        }
        let changes = self.changes();
        let mut synced = Synced::default();
        for subscription in &changes.unsubscribe {
            synced.frames.extend(session.unsubscribe(subscription));
        }
        for subscription in changes.subscribe {
            match session.subscribe(subscription.clone()) {
                Ok(frame) => synced.frames.extend(frame),
                Err(refused) => {
                    self.interests.refused(&subscription);
                    synced.refused.push((subscription, refused));
                }
            }
        }
        Ok(synced)
    }

    /// Ends the interests whose handles were dropped, and lets go of what nothing wants.
    pub(super) fn settle(&mut self) {
        if !self.interests.drain() {
            return;
        }
        let interests = &self.interests;
        self.markets.books.retain(|coin, _| interests.wants(&book(coin)));
        self.markets.tapes.retain(|coin, _| interests.wants(&Subscription::Trades { coin: coin.clone() }));
        self.markets.candles.retain(|(coin, interval), _| {
            interests.wants(&Subscription::Candle { coin: coin.clone(), interval: *interval })
        });
        let users = interests.users();
        self.accounts.retain(|user, _| users.contains(user));
        for (user, account) in &mut self.accounts {
            let user = *user;
            let orders = |dex: &str| interests.wants(&Subscription::OpenOrders { user, dex: dex.to_owned() });
            account.keep(&Keep {
                state: interests.wants(&Subscription::AllDexsClearinghouseState { user }),
                orders: &orders,
                fills: interests.wants(&Subscription::UserFills { user, aggregate_by_time: false }),
                fundings: interests.wants(&Subscription::UserFundings { user }),
                ledger: interests.wants(&Subscription::UserNonFundingLedgerUpdates { user }),
            });
        }
    }
}

#[cfg(test)]
#[path = "tests/sync.rs"]
mod tests;
