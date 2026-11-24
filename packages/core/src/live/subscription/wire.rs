//! Writing a subscription as the object a subscribe or unsubscribe frame carries
//! ([`Subscription::to_json`]), and as one line of JSON.

use std::fmt;

use crate::live::Subscription;
use crate::utils::Json;

impl Subscription {
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
            Subscription::L2 { coin, n_sig_figs, mantissa } => {
                fields.push(("c", text(coin)));
                if let Some(figures) = n_sig_figs {
                    fields.push(("s", Json::Number(figures.to_string())));
                }
                if let Some(mantissa) = mantissa {
                    fields.push(("m", Json::Number(mantissa.to_string())));
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
            | Subscription::SpotAssetCtxs
            | Subscription::Pac
            | Subscription::Sac => {}
        }
        Json::Object(fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())
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

#[cfg(test)]
#[path = "tests/wire.rs"]
mod tests;
