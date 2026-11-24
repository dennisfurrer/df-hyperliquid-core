//! Reading a subscription object back ([`Subscription::parse`]), in any key order, as the venue
//! echoes it or as it was sent.

use crate::live::{BadSubscription, Subscription};
use crate::markets::Interval;
use crate::utils::{Address, Json};

/// Every `type`: the docs' list as it was, the SDK's, then the docs' newer ones.
const KINDS: [&str; 30] = [
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
    "l2",
    "pac",
    "sac",
];

impl Subscription {
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
            "l2" => Subscription::L2 {
                coin: fields.string("c")?.to_owned(),
                n_sig_figs: fields.small("s")?,
                mantissa: fields.small("m")?,
            },
            "pac" => Subscription::Pac,
            "sac" => Subscription::Sac,
            other => return Err(BadSubscription::UnknownType(other.to_owned())),
        })
    }
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
#[path = "tests/parse.rs"]
mod tests;
