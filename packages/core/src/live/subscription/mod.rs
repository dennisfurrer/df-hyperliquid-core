//! Every subscription the venue serves (the docs' list, plus `assetCtxs`, `spotAssetCtxs` and
//! `userHistoricalOrders`), as the object a subscribe or unsubscribe frame carries. `webData2` is
//! gone: the venue refuses it.

mod parse;
mod wire;

use crate::markets::Interval;
use crate::utils::Address;

/// One feed. A coin is spelled as the venue spells it: `BTC`, `xyz:SP500` (HIP-3), `@107` or
/// `PURR/USDC` (spot), `#14720` (an outcome side). A `dex` of `""` is the first perp dex.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Subscription {
    /// Every coin's mid on one perp dex. The first dex's also carries spot and outcome mids.
    AllMids {
        /// The dex, `""` for the first.
        dex: String,
    },
    /// Short texts for the account: fills, liquidations.
    Notification {
        /// The account.
        user: Address,
    },
    /// The account as Hyperliquid's own frontend reads it.
    WebData3 {
        /// The account.
        user: Address,
    },
    /// The account's running TWAPs on one dex, on the channel `twapStates`.
    TwapStates {
        /// The account.
        user: Address,
        /// The dex, `""` for the first.
        dex: String,
    },
    /// The account's clearinghouse on one dex, on the channel `clearinghouseState`.
    ClearinghouseState {
        /// The account.
        user: Address,
        /// The dex, `""` for the first.
        dex: String,
    },
    /// The account's resting orders on one dex, on the channel `openOrders`.
    OpenOrders {
        /// The account.
        user: Address,
        /// The dex, `""` for the first.
        dex: String,
    },
    /// One coin's candles at one interval, on the channel `candle`.
    Candle {
        /// The coin.
        coin: String,
        /// The candle interval.
        interval: Interval,
    },
    /// `n_sig_figs` groups levels to 2 to 5 significant figures; `mantissa` (1, 2 or 5) groups
    /// further, with 5 figures only. `fast` gives 5 levels a side more often, not 20.
    ///
    /// How often a book comes matters more than the venue's docs suggest. They say a book is sent
    /// at most every 0.5 s, but on mainnet in November 2026 a BTC book without `fast` came about
    /// every 7 s, so a program that waits for it holds a stale book. With `fast` it came about
    /// twice a second, 5 levels a side. For the best bid and ask alone, [`Subscription::Bbo`] is
    /// sent on every block where they change (about ten times a second for BTC).
    ///
    /// For both speed and depth, subscribe twice on one feed: `bbo` or a fast book for the top of
    /// the book, and a book without `fast` for the twenty levels a side.
    ///
    /// ```
    /// use df_hyperliquid_core::live::Subscription;
    ///
    /// let coin = || "BTC".to_owned();
    /// let top = Subscription::L2Book { coin: coin(), n_sig_figs: None, mantissa: None, fast: true };
    /// let depth = Subscription::L2Book { coin: coin(), n_sig_figs: None, mantissa: None, fast: false };
    /// let touch = Subscription::Bbo { coin: coin() };
    /// # let _ = (top, depth, touch);
    /// ```
    L2Book {
        /// The coin.
        coin: String,
        /// `nSigFigs`: 2 to 5 significant figures; `None` for full precision.
        n_sig_figs: Option<u8>,
        /// `mantissa`: 1, 2 or 5, with `n_sig_figs` 5 only; `None` when not grouped further.
        mantissa: Option<u8>,
        /// `fast`: whether to get 5 levels a side more often instead of 20; sent only when true.
        fast: bool,
    },
    /// One coin's prints, on the channel `trades`.
    Trades {
        /// The coin.
        coin: String,
    },
    /// Status changes of the account's orders, on the channel `orderUpdates`.
    OrderUpdates {
        /// The account.
        user: Address,
    },
    /// Its data comes on the channel `user`.
    UserEvents {
        /// The account.
        user: Address,
    },
    /// `aggregate_by_time` merges the partial fills of one crossing order.
    UserFills {
        /// The account.
        user: Address,
        /// `aggregateByTime`: whether to merge the partial fills of one crossing order; sent only when true.
        aggregate_by_time: bool,
    },
    /// The account's funding payments, on the channel `userFundings`.
    UserFundings {
        /// The account.
        user: Address,
    },
    /// The account's ledger rows other than funding, on the channel `userNonFundingLedgerUpdates`.
    UserNonFundingLedgerUpdates {
        /// The account.
        user: Address,
    },
    /// A perp's context; a spot pair's or an outcome side's comes on `activeSpotAssetCtx`.
    ActiveAssetCtx {
        /// The coin.
        coin: String,
    },
    /// What the account may trade on a perp. Perps only.
    ActiveAssetData {
        /// The account.
        user: Address,
        /// The perp's coin.
        coin: String,
    },
    /// Fills of the account's TWAP slices, on the channel `userTwapSliceFills`.
    UserTwapSliceFills {
        /// The account.
        user: Address,
    },
    /// The account's TWAP events, on the channel `userTwapHistory`.
    UserTwapHistory {
        /// The account.
        user: Address,
    },
    /// The best bid and offer, sent only when one of them moves.
    Bbo {
        /// The coin.
        coin: String,
    },
    /// The docs name the option `isPortfolioMargin`; the venue ignores that and echoes
    /// `ignorePortfolioMargin`, which is what it reads.
    SpotState {
        /// The account.
        user: Address,
        /// `ignorePortfolioMargin`, the option the venue reads; sent only when true.
        ignore_portfolio_margin: bool,
    },
    /// The account's clearinghouse on every dex, on the channel `allDexsClearinghouseState`.
    AllDexsClearinghouseState {
        /// The account.
        user: Address,
    },
    /// Every perp dex's contexts in one frame.
    AllDexsAssetCtxs,
    /// Changes to `outcomeMeta`, sent only when something changes.
    OutcomeMetaUpdates,
    /// Mark and mid changes, base64 of raw DEFLATE.
    FastAssetCtxs,
    /// One perp dex's contexts. In `@nktkas/hyperliquid`, not on the docs' list.
    AssetCtxs {
        /// The dex, `""` for the first.
        dex: String,
    },
    /// Every spot and outcome context. In `@nktkas/hyperliquid`, not on the docs' list.
    SpotAssetCtxs,
    /// Orders and what became of them. In `@nktkas/hyperliquid`, not on the docs' list.
    UserHistoricalOrders {
        /// The account.
        user: Address,
    },
    /// The book as a snapshot of up to twenty levels a side, then diffs. `n_sig_figs` and
    /// `mantissa` group levels as on [`Subscription::L2Book`]; on the wire they are `s` and `m`, and
    /// the coin is `c`.
    L2 {
        /// `c`: the coin.
        coin: String,
        /// `s`: 2 to 5 significant figures; `None` for full precision.
        n_sig_figs: Option<u8>,
        /// `m`: 1, 2 or 5, with `n_sig_figs` 5 only; `None` when not grouped further.
        mantissa: Option<u8>,
    },
    /// Every perp dex's contexts, base64 of raw DEFLATE: a snapshot, then only what changed.
    Pac,
    /// Every spot pair's and outcome side's context by coin, base64 of raw DEFLATE: a snapshot,
    /// then only what changed.
    Sac,
}

/// Why an object is not a subscription, or a subscription is not one the venue would take.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BadSubscription {
    /// The value is not a JSON object.
    #[error("a subscription is an object")]
    NotAnObject,
    /// The `type` names no subscription the venue serves; the `type` as written.
    #[error("`{0}` is not a subscription the venue serves")]
    UnknownType(String),
    /// A field the subscription needs is not there.
    #[error("`{kind}` needs `{field}`")]
    Missing {
        /// The subscription's `type`, or `subscription` when the `type` itself is at fault.
        kind: &'static str,
        /// The missing field's wire name.
        field: &'static str,
    },
    /// A field is there but of the wrong JSON type.
    #[error("`{field}` of `{kind}` must be {expected}")]
    Wrong {
        /// The subscription's `type`, or `subscription` when the `type` itself is at fault.
        kind: &'static str,
        /// The field's wire name.
        field: &'static str,
        /// What the field must be, in words.
        expected: &'static str,
    },
    /// The coin is `""`.
    #[error("a coin is never an empty string")]
    EmptyCoin,
    /// `nSigFigs` is not 2, 3, 4 or 5; the value given.
    #[error("nSigFigs is 2, 3, 4 or 5, not {0}")]
    SigFigs(u8),
    /// `mantissa` is not 1, 2 or 5, or comes without `nSigFigs` 5; the value given.
    #[error("mantissa is 1, 2 or 5, and only with nSigFigs 5; {0} is refused here")]
    Mantissa(u8),
}

impl Subscription {
    /// Its `type`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Subscription::AllMids { .. } => "allMids",
            Subscription::Notification { .. } => "notification",
            Subscription::WebData3 { .. } => "webData3",
            Subscription::TwapStates { .. } => "twapStates",
            Subscription::ClearinghouseState { .. } => "clearinghouseState",
            Subscription::OpenOrders { .. } => "openOrders",
            Subscription::Candle { .. } => "candle",
            Subscription::L2Book { .. } => "l2Book",
            Subscription::Trades { .. } => "trades",
            Subscription::OrderUpdates { .. } => "orderUpdates",
            Subscription::UserEvents { .. } => "userEvents",
            Subscription::UserFills { .. } => "userFills",
            Subscription::UserFundings { .. } => "userFundings",
            Subscription::UserNonFundingLedgerUpdates { .. } => "userNonFundingLedgerUpdates",
            Subscription::ActiveAssetCtx { .. } => "activeAssetCtx",
            Subscription::ActiveAssetData { .. } => "activeAssetData",
            Subscription::UserTwapSliceFills { .. } => "userTwapSliceFills",
            Subscription::UserTwapHistory { .. } => "userTwapHistory",
            Subscription::Bbo { .. } => "bbo",
            Subscription::SpotState { .. } => "spotState",
            Subscription::AllDexsClearinghouseState { .. } => "allDexsClearinghouseState",
            Subscription::AllDexsAssetCtxs => "allDexsAssetCtxs",
            Subscription::OutcomeMetaUpdates => "outcomeMetaUpdates",
            Subscription::FastAssetCtxs => "fastAssetCtxs",
            Subscription::AssetCtxs { .. } => "assetCtxs",
            Subscription::SpotAssetCtxs => "spotAssetCtxs",
            Subscription::UserHistoricalOrders { .. } => "userHistoricalOrders",
            Subscription::L2 { .. } => "l2",
            Subscription::Pac => "pac",
            Subscription::Sac => "sac",
        }
    }

    /// The channel its data comes on: its `type`, except `userEvents` (on `user`) and a spot or
    /// outcome coin's `activeAssetCtx` (on `activeSpotAssetCtx`), so matching on the name sent
    /// would never fire.
    #[must_use]
    pub fn channel(&self) -> &'static str {
        match self {
            Subscription::UserEvents { .. } => "user",
            Subscription::ActiveAssetCtx { coin } if is_spot(coin) => "activeSpotAssetCtx",
            other => other.kind(),
        }
    }

    /// The account a user subscription follows. The venue allows ten across an IP's
    /// subscriptions.
    #[must_use]
    pub const fn user(&self) -> Option<Address> {
        match self {
            Subscription::Notification { user }
            | Subscription::WebData3 { user }
            | Subscription::TwapStates { user, .. }
            | Subscription::ClearinghouseState { user, .. }
            | Subscription::OpenOrders { user, .. }
            | Subscription::OrderUpdates { user }
            | Subscription::UserEvents { user }
            | Subscription::UserFills { user, .. }
            | Subscription::UserFundings { user }
            | Subscription::UserNonFundingLedgerUpdates { user }
            | Subscription::ActiveAssetData { user, .. }
            | Subscription::UserTwapSliceFills { user }
            | Subscription::UserTwapHistory { user }
            | Subscription::SpotState { user, .. }
            | Subscription::AllDexsClearinghouseState { user }
            | Subscription::UserHistoricalOrders { user } => Some(*user),
            _ => None,
        }
    }

    /// The coin it follows, for the subscriptions that follow one.
    #[must_use]
    pub fn coin(&self) -> Option<&str> {
        match self {
            Subscription::Candle { coin, .. }
            | Subscription::L2Book { coin, .. }
            | Subscription::L2 { coin, .. }
            | Subscription::Trades { coin }
            | Subscription::ActiveAssetCtx { coin }
            | Subscription::ActiveAssetData { coin, .. }
            | Subscription::Bbo { coin } => Some(coin),
            _ => None,
        }
    }

    /// Whether the venue would take it: a coin is named, and a book's grouping is one the info
    /// docs allow. The venue drops the whole socket for some subscriptions it cannot serve (a
    /// `bbo` of an unknown coin), so a coin should come from the venue's own lists.
    pub fn check(&self) -> Result<(), BadSubscription> {
        if self.coin().is_some_and(str::is_empty) {
            return Err(BadSubscription::EmptyCoin);
        }
        if let Subscription::L2Book { n_sig_figs, mantissa, .. } | Subscription::L2 { n_sig_figs, mantissa, .. } = self
        {
            if let Some(figures) = n_sig_figs.filter(|figures| !(2..=5).contains(figures)) {
                return Err(BadSubscription::SigFigs(figures));
            }
            if let Some(mantissa) = mantissa.filter(|mantissa| *n_sig_figs != Some(5) || ![1, 2, 5].contains(mantissa))
            {
                return Err(BadSubscription::Mantissa(mantissa));
            }
        }
        Ok(())
    }
}

/// A spot pair (`@107`, `PURR/USDC`) or an outcome side (`#14720`, `+14720`), by its spelling.
fn is_spot(coin: &str) -> bool {
    coin.starts_with(['@', '#', '+']) || coin.contains('/')
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "tests/subscription.rs"]
mod tests;
