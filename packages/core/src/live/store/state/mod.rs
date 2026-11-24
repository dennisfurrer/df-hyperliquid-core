//! The store: one network's live state, the interests that keep it live, and its views. The
//! [`Store`] and what it takes and gives are here; syncing a session, folding frames and reads
//! (market data, account data) and the views each have a file of their own.

mod account_data;
mod apply;
mod market_data;
mod sync;
mod views;

use std::collections::BTreeMap;

use crate::account::history::{Fill, LedgerUpdate};
use crate::account::{Abstraction, ClearinghouseState, FrontendOrder};
use crate::client::Network;
use crate::live::store::Account;
use crate::live::store::interest::Interests;
use crate::live::store::market::Markets;
use crate::live::{Limits, Refused, Subscription};
use crate::markets::perps::{MetaAndAssetCtxs, UserFunding};
use crate::markets::spot::{SpotMeta, SpotMetaAndAssetCtxs};
use crate::markets::{Book, Candle, Mids, Trade};
use crate::utils::Address;

/// A typed `/info` answer for the store to fold.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Read<'a> {
    /// `l2Book`, ungrouped.
    Book(&'a Book),
    /// `recentTrades`, newest first.
    Trades(&'a [Trade]),
    /// `candleSnapshot`, oldest first: what the socket already sent of the same candles stays.
    Candles(&'a [Candle]),
    /// `allMids` of one perp dex, `""` the first.
    Mids {
        /// The dex, `""` for the first perp dex.
        dex: &'a str,
        /// The mids.
        mids: &'a Mids,
    },
    /// `metaAndAssetCtxs`: the first dex's universe, collateral and contexts.
    Perps(&'a MetaAndAssetCtxs),
    /// `metaAndAssetCtxs` with a HIP-3 `dex`.
    Dex {
        /// The HIP-3 dex's name.
        dex: &'a str,
        /// The answer.
        read: &'a MetaAndAssetCtxs,
    },
    /// `perpDexs` with `allPerpMetas`: every dex's universe and collateral.
    Dexes(&'a crate::markets::dexes::Dexes),
    /// `spotMetaAndAssetCtxs`.
    Spot(&'a SpotMetaAndAssetCtxs),
    /// `spotMeta`: the tokens and pairs that price spot balances.
    SpotMeta(&'a SpotMeta),
    /// `clearinghouseState` of one dex, at the account's own address.
    Clearinghouse {
        /// The account it was read for.
        user: Address,
        /// The dex, `""` for the first perp dex.
        dex: &'a str,
        /// The answer.
        state: &'a ClearinghouseState,
    },
    /// `spotClearinghouseState`.
    SpotState {
        /// The account it was read for.
        user: Address,
        /// The answer.
        state: &'a crate::account::SpotState,
    },
    /// `userAbstraction`.
    Mode {
        /// The account it was read for.
        user: Address,
        /// The account mode.
        mode: &'a Abstraction,
    },
    /// `frontendOpenOrders` of one dex.
    Orders {
        /// The account it was read for.
        user: Address,
        /// The dex, `""` for the first perp dex.
        dex: &'a str,
        /// The resting orders.
        orders: &'a [FrontendOrder],
    },
    /// `userFills`: the latest, newest first, which replace what is held.
    Fills {
        /// The account it was read for.
        user: Address,
        /// The fills, newest first.
        fills: &'a [Fill],
    },
    /// `userFillsByTime`: a window, oldest first, added to what is held.
    MoreFills {
        /// The account it was read for.
        user: Address,
        /// The fills, oldest first.
        fills: &'a [Fill],
    },
    /// `userFunding`: a window, oldest first, added to what is held.
    Fundings {
        /// The account it was read for.
        user: Address,
        /// The funding payments, oldest first.
        fundings: &'a [UserFunding],
    },
    /// `userNonFundingLedgerUpdates`: a window, oldest first, added to what is held, in the
    /// socket's row type.
    Ledger {
        /// The account it was read for.
        user: Address,
        /// The ledger rows, oldest first.
        rows: &'a [LedgerUpdate],
    },
}

/// What a frame or a read did to the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Folded {
    /// Something a view reads changed.
    Changed,
    /// Told again, and nothing changed but when it was heard.
    Same,
    /// Not kept: nothing wants it, it is older than what is held, or it is not state.
    Dropped,
}

/// What [`Store::sync`] gives the socket's owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Synced {
    /// The frames to send, in order.
    pub frames: Vec<String>,
    /// What the session would not take; asked for again on the next sync.
    pub refused: Vec<(Subscription, Refused)>,
}

/// A store and a session on two networks: one store is one network and never feeds a session on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this store is on {store} and the session on {session}; one store is one network")]
#[non_exhaustive]
pub struct OtherNetwork {
    /// The store's network.
    pub store: Network,
    /// The session's network.
    pub session: Network,
}

/// One network's live state.
#[derive(Debug)]
pub struct Store {
    network: Network,
    interests: Interests,
    markets: Markets,
    accounts: BTreeMap<Address, Account>,
}

impl Store {
    /// An empty store on `network`, within the venue's whole per-IP limits.
    #[must_use]
    pub fn new(network: Network) -> Store {
        Store {
            network,
            interests: Interests::new(Limits::VENUE),
            markets: Markets::default(),
            accounts: BTreeMap::new(),
        }
    }

    /// Within a part of the IP's limits, for a store sharing it with other sessions.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Store {
        self.interests.set_limits(limits);
        self
    }

    /// The network the store is on.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }
}

impl Folded {
    const fn of(changed: bool) -> Folded {
        match changed {
            true => Folded::Changed,
            false => Folded::Same,
        }
    }

    /// The more of the two: a change over a repeat over nothing kept.
    const fn or(self, other: Folded) -> Folded {
        match (self, other) {
            (Folded::Changed, _) | (_, Folded::Changed) => Folded::Changed,
            (Folded::Same, _) | (_, Folded::Same) => Folded::Same,
            _ => Folded::Dropped,
        }
    }
}

/// The full book's subscription, the one [`Interest::Book`](crate::live::store::Interest::Book)
/// asks for.
fn book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}
