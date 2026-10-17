//! Every subscription the venue serves (the docs' list, plus `assetCtxs`, `spotAssetCtxs` and
//! `userHistoricalOrders`), as the object a subscribe or unsubscribe frame carries. `webData2` is
//! gone: the venue refuses it.

use std::fmt;

use crate::markets::candle::Interval;
use crate::utils::{Address, Json};

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

/// Every `type`, in the docs' order and then the SDK's.
const KINDS: [&str; 27] = [
    "allMids",
    "notification",
    "webData3",
    "twapStates",
    "clearinghouseState",
    "openOrders",
    "candle",
    "l2Book",
    "trades",
    "orderUpdates",
    "userEvents",
    "userFills",
    "userFundings",
    "userNonFundingLedgerUpdates",
    "activeAssetCtx",
    "activeAssetData",
    "userTwapSliceFills",
    "userTwapHistory",
    "bbo",
    "spotState",
    "allDexsClearinghouseState",
    "allDexsAssetCtxs",
    "outcomeMetaUpdates",
    "fastAssetCtxs",
    "assetCtxs",
    "spotAssetCtxs",
    "userHistoricalOrders",
];

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
            | Subscription::Trades { coin }
            | Subscription::ActiveAssetCtx { coin }
            | Subscription::ActiveAssetData { coin, .. }
            | Subscription::Bbo { coin } => Some(coin),
            _ => None,
        }
    }

    /// The object a subscribe or unsubscribe frame carries: keys in the docs' order, optional ones
    /// last and only when set, as the Python SDK sends them. The venue echoes left out options
    /// filled in (`fast: false`, `dex: ""`), and [`Subscription::parse`] reads that echo back to
    /// the same subscription.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let mut fields = vec![("type", text(self.kind()))];
        match self {
            Subscription::AllMids { dex } if !dex.is_empty() => fields.push(("dex", text(dex))),
            Subscription::TwapStates { user, dex }
            | Subscription::ClearinghouseState { user, dex }
            | Subscription::OpenOrders { user, dex } => {
                fields.push(("user", text(&user.to_hex())));
                fields.push(("dex", text(dex)));
            }
            Subscription::Candle { coin, interval } => {
                fields.push(("coin", text(coin)));
                fields.push(("interval", text(interval.wire())));
            }
            Subscription::L2Book { coin, n_sig_figs, mantissa, fast } => {
                fields.push(("coin", text(coin)));
                if let Some(figures) = n_sig_figs {
                    fields.push(("nSigFigs", Json::Number(figures.to_string())));
                }
                if let Some(mantissa) = mantissa {
                    fields.push(("mantissa", Json::Number(mantissa.to_string())));
                }
                if *fast {
                    fields.push(("fast", Json::Bool(true)));
                }
            }
            Subscription::Trades { coin } | Subscription::ActiveAssetCtx { coin } | Subscription::Bbo { coin } => {
                fields.push(("coin", text(coin)));
            }
            Subscription::UserFills { user, aggregate_by_time } => {
                fields.push(("user", text(&user.to_hex())));
                if *aggregate_by_time {
                    fields.push(("aggregateByTime", Json::Bool(true)));
                }
            }
            Subscription::ActiveAssetData { user, coin } => {
                fields.push(("user", text(&user.to_hex())));
                fields.push(("coin", text(coin)));
            }
            Subscription::SpotState { user, ignore_portfolio_margin } => {
                fields.push(("user", text(&user.to_hex())));
                if *ignore_portfolio_margin {
                    fields.push(("ignorePortfolioMargin", Json::Bool(true)));
                }
            }
            Subscription::AssetCtxs { dex } => fields.push(("dex", text(dex))),
            Subscription::Notification { user }
            | Subscription::WebData3 { user }
            | Subscription::OrderUpdates { user }
            | Subscription::UserEvents { user }
            | Subscription::UserFundings { user }
            | Subscription::UserNonFundingLedgerUpdates { user }
            | Subscription::UserTwapSliceFills { user }
            | Subscription::UserTwapHistory { user }
            | Subscription::AllDexsClearinghouseState { user }
            | Subscription::UserHistoricalOrders { user } => fields.push(("user", text(&user.to_hex()))),
            Subscription::AllMids { .. }
            | Subscription::AllDexsAssetCtxs
            | Subscription::OutcomeMetaUpdates
            | Subscription::FastAssetCtxs
            | Subscription::SpotAssetCtxs => {}
        }
        Json::Object(fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
    }

    /// Whether the venue would take it: a coin is named, and a book's grouping is one the info
    /// docs allow. The venue drops the whole socket for some subscriptions it cannot serve (a
    /// `bbo` of an unknown coin), so a coin should come from the venue's own lists.
    pub fn check(&self) -> Result<(), BadSubscription> {
        if self.coin().is_some_and(str::is_empty) {
            return Err(BadSubscription::EmptyCoin);
        }
        if let Subscription::L2Book { n_sig_figs, mantissa, .. } = self {
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

    /// Reads a subscription object, in any key order, as the venue echoes it or as it was sent.
    /// Keys it does not know are passed over; `null`, `false` and `""` read as left out.
    pub fn parse(json: &Json) -> Result<Subscription, BadSubscription> {
        if !matches!(json, Json::Object(_)) {
            return Err(BadSubscription::NotAnObject);
        }
        let named = json.get("type").ok_or(BadSubscription::Missing { kind: "subscription", field: "type" })?;
        let named = named.as_str().ok_or(BadSubscription::Wrong {
            kind: "subscription",
            field: "type",
            expected: "a string",
        })?;
        let kind = KINDS
            .into_iter()
            .find(|kind| *kind == named)
            .ok_or_else(|| BadSubscription::UnknownType(named.to_owned()))?;
        let fields = Fields { json, kind };
        Ok(match kind {
            "allMids" => Subscription::AllMids { dex: fields.dex()? },
            "notification" => Subscription::Notification { user: fields.user()? },
            "webData3" => Subscription::WebData3 { user: fields.user()? },
            "twapStates" => Subscription::TwapStates { user: fields.user()?, dex: fields.dex()? },
            "clearinghouseState" => Subscription::ClearinghouseState { user: fields.user()?, dex: fields.dex()? },
            "openOrders" => Subscription::OpenOrders { user: fields.user()?, dex: fields.dex()? },
            "candle" => Subscription::Candle { coin: fields.coin()?, interval: fields.interval()? },
            "l2Book" => Subscription::L2Book {
                coin: fields.coin()?,
                n_sig_figs: fields.small("nSigFigs")?,
                mantissa: fields.small("mantissa")?,
                fast: fields.flag("fast")?,
            },
            "trades" => Subscription::Trades { coin: fields.coin()? },
            "orderUpdates" => Subscription::OrderUpdates { user: fields.user()? },
            "userEvents" => Subscription::UserEvents { user: fields.user()? },
            "userFills" => {
                Subscription::UserFills { user: fields.user()?, aggregate_by_time: fields.flag("aggregateByTime")? }
            }
            "userFundings" => Subscription::UserFundings { user: fields.user()? },
            "userNonFundingLedgerUpdates" => Subscription::UserNonFundingLedgerUpdates { user: fields.user()? },
            "activeAssetCtx" => Subscription::ActiveAssetCtx { coin: fields.coin()? },
            "activeAssetData" => Subscription::ActiveAssetData { user: fields.user()?, coin: fields.coin()? },
            "userTwapSliceFills" => Subscription::UserTwapSliceFills { user: fields.user()? },
            "userTwapHistory" => Subscription::UserTwapHistory { user: fields.user()? },
            "bbo" => Subscription::Bbo { coin: fields.coin()? },
            "spotState" => Subscription::SpotState {
                user: fields.user()?,
                ignore_portfolio_margin: fields.flag("ignorePortfolioMargin")?,
            },
            "allDexsClearinghouseState" => Subscription::AllDexsClearinghouseState { user: fields.user()? },
            "allDexsAssetCtxs" => Subscription::AllDexsAssetCtxs,
            "outcomeMetaUpdates" => Subscription::OutcomeMetaUpdates,
            "fastAssetCtxs" => Subscription::FastAssetCtxs,
            "assetCtxs" => Subscription::AssetCtxs { dex: fields.dex()? },
            "spotAssetCtxs" => Subscription::SpotAssetCtxs,
            "userHistoricalOrders" => Subscription::UserHistoricalOrders { user: fields.user()? },
            other => return Err(BadSubscription::UnknownType(other.to_owned())),
        })
    }
}

impl fmt::Display for Subscription {
    /// The object as one line of JSON.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json().compact())
    }
}

fn text(value: &str) -> Json {
    Json::String(value.to_owned())
}

/// A spot pair (`@107`, `PURR/USDC`) or an outcome side (`#14720`, `+14720`), by its spelling.
fn is_spot(coin: &str) -> bool {
    coin.starts_with(['@', '#', '+']) || coin.contains('/')
}

/// The fields of one subscription object, read for the `kind` it names.
struct Fields<'a> {
    json: &'a Json,
    kind: &'static str,
}

impl Fields<'_> {
    fn wrong(&self, field: &'static str, expected: &'static str) -> BadSubscription {
        BadSubscription::Wrong { kind: self.kind, field, expected }
    }

    /// A field that is there and not `null`.
    fn present(&self, field: &str) -> Option<&Json> {
        self.json.get(field).filter(|value| !matches!(value, Json::Null))
    }

    fn string(&self, field: &'static str) -> Result<&str, BadSubscription> {
        let value = self.present(field).ok_or(BadSubscription::Missing { kind: self.kind, field })?;
        value.as_str().ok_or_else(|| self.wrong(field, "a string"))
    }

    fn user(&self) -> Result<Address, BadSubscription> {
        self.string("user")?.parse().map_err(|_| self.wrong("user", "an address"))
    }

    fn coin(&self) -> Result<String, BadSubscription> {
        self.string("coin").map(ToOwned::to_owned)
    }

    fn interval(&self) -> Result<Interval, BadSubscription> {
        Interval::parse(self.string("interval")?).ok_or_else(|| self.wrong("interval", "an interval the docs list"))
    }

    fn dex(&self) -> Result<String, BadSubscription> {
        match self.present("dex") {
            None => Ok(String::new()),
            Some(value) => value.as_str().map(ToOwned::to_owned).ok_or_else(|| self.wrong("dex", "a string")),
        }
    }

    fn flag(&self, field: &'static str) -> Result<bool, BadSubscription> {
        match self.present(field) {
            None => Ok(false),
            Some(value) => value.as_bool().ok_or_else(|| self.wrong(field, "true or false")),
        }
    }

    fn small(&self, field: &'static str) -> Result<Option<u8>, BadSubscription> {
        match self.present(field) {
            None => Ok(None),
            Some(value) => value
                .as_u64()
                .and_then(|number| u8::try_from(number).ok())
                .map(Some)
                .ok_or_else(|| self.wrong(field, "a small whole number")),
        }
    }
}

#[cfg(test)]
#[path = "tests/subscription.rs"]
mod tests;
