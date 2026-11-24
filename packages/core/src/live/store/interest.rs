//! What readers need kept live, counted, and the subscriptions that serve it. A subscription is
//! wanted while any interest needs it; letting go and taking hold again before the next
//! [`Store::changes`](crate::live::store::Store::changes) costs nothing on the socket.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::mpsc::{self, Receiver, Sender},
};

use crate::live::{Limits, Refused, Subscription};
use crate::markets::Interval;
use crate::utils::Address;

/// What a reader needs kept live. A coin is spelled as the venue spells it: `BTC`, `xyz:XYZ100`
/// (HIP-3), `@107` or `PURR/USDC` (spot), `#14720` (an outcome side).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Interest {
    /// A market's whole book, ungrouped: its frames name no grouping, so one book a coin.
    Book {
        /// The coin.
        coin: String,
    },
    /// A market's prints.
    Trades {
        /// The coin.
        coin: String,
    },
    /// A market's candles at one interval.
    Candles {
        /// The coin.
        coin: String,
        /// The candle interval.
        interval: Interval,
    },
    /// Every coin's mid on one perp dex, `""` the first, which also carries spot and outcomes.
    Mids {
        /// The dex, `""` for the first.
        dex: String,
    },
    /// One market's context: a perp's, a spot pair's or an outcome side's.
    Context {
        /// The coin.
        coin: String,
    },
    /// Every perp's context on every dex, in one frame.
    PerpContexts,
    /// Every spot pair's and outcome side's context, in one frame.
    SpotContexts,
    /// An account's clearinghouse on every dex, its spot balances and its mode: what its
    /// positions, usable balances and worth are read from.
    State {
        /// The account.
        user: Address,
    },
    /// An account's resting orders on one dex; spot and outcome orders come with the first.
    Orders {
        /// The account.
        user: Address,
        /// The dex, `""` for the first.
        dex: String,
    },
    /// An account's fills.
    Fills {
        /// The account.
        user: Address,
    },
    /// An account's funding payments.
    Fundings {
        /// The account.
        user: Address,
    },
    /// Deposits, withdrawals, transfers and every other ledger row but funding.
    Ledger {
        /// The account.
        user: Address,
    },
}

impl Interest {
    /// The subscriptions that serve it.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<Subscription> {
        match self {
            Interest::Book { coin } => {
                vec![Subscription::L2Book { coin: coin.clone(), n_sig_figs: None, mantissa: None, fast: false }]
            }
            Interest::Trades { coin } => vec![Subscription::Trades { coin: coin.clone() }],
            Interest::Candles { coin, interval } => {
                vec![Subscription::Candle { coin: coin.clone(), interval: *interval }]
            }
            Interest::Mids { dex } => vec![Subscription::AllMids { dex: dex.clone() }],
            Interest::Context { coin } => vec![Subscription::ActiveAssetCtx { coin: coin.clone() }],
            Interest::PerpContexts => vec![Subscription::AllDexsAssetCtxs],
            Interest::SpotContexts => vec![Subscription::SpotAssetCtxs],
            Interest::State { user } => vec![
                Subscription::AllDexsClearinghouseState { user: *user },
                Subscription::SpotState { user: *user, ignore_portfolio_margin: false },
                Subscription::WebData3 { user: *user },
            ],
            Interest::Orders { user, dex } => vec![Subscription::OpenOrders { user: *user, dex: dex.clone() }],
            Interest::Fills { user } => vec![Subscription::UserFills { user: *user, aggregate_by_time: false }],
            Interest::Fundings { user } => vec![Subscription::UserFundings { user: *user }],
            Interest::Ledger { user } => vec![Subscription::UserNonFundingLedgerUpdates { user: *user }],
        }
    }

    /// The account it follows, for the ones that follow one.
    #[must_use]
    pub const fn user(&self) -> Option<Address> {
        match self {
            Interest::State { user }
            | Interest::Orders { user, .. }
            | Interest::Fills { user }
            | Interest::Fundings { user }
            | Interest::Ledger { user } => Some(*user),
            _ => None,
        }
    }
}

/// A hold on an interest. Dropping it ends the interest, as
/// [`Store::release`](crate::live::store::Store::release) does.
#[must_use = "dropping a handle ends its interest"]
#[derive(Debug)]
pub struct Handle {
    id: u64,
    interest: Interest,
    released: Sender<u64>,
}

impl Handle {
    /// The interest it holds.
    #[must_use]
    pub const fn interest(&self) -> &Interest {
        &self.interest
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // The store may be gone already; then there is nothing left to end.
        let _ = self.released.send(self.id);
    }
}

/// What to tell the socket: what nothing needs any more, then what is newly needed, each in the
/// order it was first wanted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Changes {
    /// Subscriptions to drop, sent first.
    pub unsubscribe: Vec<Subscription>,
    /// Subscriptions to add.
    pub subscribe: Vec<Subscription>,
}

impl Changes {
    /// Whether there is nothing to send.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.unsubscribe.is_empty() && self.subscribe.is_empty()
    }
}

/// Every interest held, and the subscriptions they add up to.
#[derive(Debug)]
pub(crate) struct Interests {
    limits: Limits,
    next: u64,
    held: BTreeMap<u64, Interest>,
    /// How many interests need each subscription.
    counts: HashMap<Subscription, usize>,
    /// The wanted subscriptions, in the order each was first wanted.
    order: Vec<Subscription>,
    /// What the socket was last told to hold.
    active: Vec<Subscription>,
    /// The accounts followed, in the order first followed.
    users: Vec<Address>,
    released: Receiver<u64>,
    sender: Sender<u64>,
}

impl Interests {
    pub(crate) fn new(limits: Limits) -> Interests {
        let (sender, released) = mpsc::channel();
        Interests {
            limits,
            next: 1,
            held: BTreeMap::new(),
            counts: HashMap::new(),
            order: Vec::new(),
            active: Vec::new(),
            users: Vec::new(),
            released,
            sender,
        }
    }

    pub(crate) fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Holds `interest`, unless what it needs would take the subscriptions or the users past the
    /// limits, or the venue would not take one of them; then nothing of it is held.
    pub(crate) fn register(&mut self, interest: Interest) -> Result<Handle, Refused> {
        let needed = interest.subscriptions();
        let mut new: Vec<&Subscription> = Vec::new();
        for subscription in &needed {
            subscription.check()?;
            if !self.counts.contains_key(subscription) && !new.contains(&subscription) {
                new.push(subscription);
            }
        }
        if self.order.len().saturating_add(new.len()) > self.limits.subscriptions {
            return Err(Refused::Subscriptions { limit: self.limits.subscriptions });
        }
        if let Some(user) = interest.user()
            && !self.users.contains(&user)
        {
            if self.users.len() >= self.limits.users {
                return Err(Refused::Users { limit: self.limits.users });
            }
            self.users.push(user);
        }
        for subscription in needed {
            let count = self.counts.entry(subscription.clone()).or_insert(0);
            if *count == 0 {
                self.order.push(subscription);
            }
            *count = count.saturating_add(1);
        }
        let id = self.next;
        self.next = self.next.saturating_add(1);
        self.held.insert(id, interest.clone());
        Ok(Handle { id, interest, released: self.sender.clone() })
    }

    /// Ends every interest whose handle was dropped. Says whether any was.
    pub(crate) fn drain(&mut self) -> bool {
        let mut any = false;
        while let Ok(id) = self.released.try_recv() {
            let Some(interest) = self.held.remove(&id) else { continue };
            any = true;
            for subscription in interest.subscriptions() {
                let Some(count) = self.counts.get_mut(&subscription) else { continue };
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.counts.remove(&subscription);
                    self.order.retain(|wanted| *wanted != subscription);
                }
            }
        }
        let order = &self.order;
        self.users.retain(|user| order.iter().any(|wanted| wanted.user() == Some(*user)));
        any
    }

    /// Whether an interest needs `subscription` now.
    pub(crate) fn wants(&self, subscription: &Subscription) -> bool {
        self.counts.contains_key(subscription)
    }

    /// Every subscription an interest needs, in the order first wanted.
    pub(crate) fn wanted(&self) -> &[Subscription] {
        &self.order
    }

    /// What changed since the socket was last told, which it now is.
    pub(crate) fn changes(&mut self) -> Changes {
        let held: HashSet<&Subscription> = self.active.iter().collect();
        let unsubscribe = self.active.iter().filter(|held| !self.counts.contains_key(*held)).cloned().collect();
        let subscribe = self.order.iter().filter(|wanted| !held.contains(wanted)).cloned().collect();
        self.active.clone_from(&self.order);
        Changes { unsubscribe, subscribe }
    }

    /// A subscription the socket refused: no longer held there, so the next changes ask again.
    pub(crate) fn refused(&mut self, subscription: &Subscription) {
        self.active.retain(|held| held != subscription);
    }

    /// The accounts the wanted subscriptions follow, in the order first followed.
    pub(crate) fn users(&self) -> &[Address] {
        &self.users
    }
}

#[cfg(test)]
#[path = "tests/interest.rs"]
mod tests;
