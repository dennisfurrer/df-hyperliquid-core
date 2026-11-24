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
pub enum Subscription {
    /// Every coin's mid on one perp dex. The first dex's also carries spot and outcome mids.
    AllMids {
        dex: String,
    },
    /// Short texts for the account: fills, liquidations.
    Notification {
        user: Address,
    },
    /// The account as Hyperliquid's own frontend reads it.
    WebData3 {
        user: Address,
    },
    TwapStates {
        user: Address,
        dex: String,
    },
    ClearinghouseState {
        user: Address,
        dex: String,
    },
    OpenOrders {
        user: Address,
        dex: String,
    },
    Candle {
        coin: String,
        interval: Interval,
    },
    /// `n_sig_figs` groups levels to 2 to 5 significant figures; `mantissa` (1, 2 or 5) groups
    /// further, with 5 figures only. `fast` gives 5 levels a side more often, not 20.
    L2Book {
        coin: String,
        n_sig_figs: Option<u8>,
        mantissa: Option<u8>,
        fast: bool,
    },
    Trades {
        coin: String,
    },
    OrderUpdates {
        user: Address,
    },
    /// Its data comes on the channel `user`.
    UserEvents {
        user: Address,
    },
    /// `aggregate_by_time` merges the partial fills of one crossing order.
    UserFills {
        user: Address,
        aggregate_by_time: bool,
    },
    UserFundings {
        user: Address,
    },
    UserNonFundingLedgerUpdates {
        user: Address,
    },
    /// A perp's context; a spot pair's or an outcome side's comes on `activeSpotAssetCtx`.
    ActiveAssetCtx {
        coin: String,
    },
    /// What the account may trade on a perp. Perps only.
    ActiveAssetData {
        user: Address,
        coin: String,
    },
    UserTwapSliceFills {
        user: Address,
    },
    UserTwapHistory {
        user: Address,
    },
    /// The best bid and offer, sent only when one of them moves.
    Bbo {
        coin: String,
    },
    /// The docs name the option `isPortfolioMargin`; the venue ignores that and echoes
    /// `ignorePortfolioMargin`, which is what it reads.
    SpotState {
        user: Address,
        ignore_portfolio_margin: bool,
    },
    AllDexsClearinghouseState {
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
        dex: String,
    },
    /// Every spot and outcome context. In `@nktkas/hyperliquid`, not on the docs' list.
    SpotAssetCtxs,
    /// Orders and what became of them. In `@nktkas/hyperliquid`, not on the docs' list.
    UserHistoricalOrders {
        user: Address,
    },
    /// The book as a snapshot of up to twenty levels a side, then diffs. `n_sig_figs` and
    /// `mantissa` group levels as on [`Subscription::L2Book`]; on the wire they are `s` and `m`, and
    /// the coin is `c`.
    L2 {
        coin: String,
        n_sig_figs: Option<u8>,
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
pub enum BadSubscription {
    #[error("a subscription is an object")]
    NotAnObject,
    #[error("`{0}` is not a subscription the venue serves")]
    UnknownType(String),
    #[error("`{kind}` needs `{field}`")]
    Missing { kind: &'static str, field: &'static str },
    #[error("`{field}` of `{kind}` must be {expected}")]
    Wrong { kind: &'static str, field: &'static str, expected: &'static str },
    #[error("a coin is never an empty string")]
    EmptyCoin,
    #[error("nSigFigs is 2, 3, 4 or 5, not {0}")]
    SigFigs(u8),
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
