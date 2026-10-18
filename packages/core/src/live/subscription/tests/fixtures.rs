//! What the subscription tests share: every subscription with the exact object sent for it.

use crate::live::Subscription;
use crate::live::fixtures::{FIRST, SECOND};
use crate::markets::Interval;
use crate::utils::Address;

pub fn user(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn coin(text: &str) -> String {
    text.to_owned()
}

pub fn book(name: &str) -> Subscription {
    Subscription::L2Book { coin: coin(name), n_sig_figs: None, mantissa: None, fast: false }
}

/// Every subscription with the exact object sent for it on mainnet on 2026-10-08, each of which
/// the venue acknowledged. Keys are in the order the docs' subscription messages write them,
/// optional ones last and only when set, as the Python SDK sends them.
pub fn sent() -> Vec<(Subscription, String)> {
    let (a, b) = (user(FIRST), user(SECOND));
    vec![
        (Subscription::AllMids { dex: String::new() }, r#"{"type":"allMids"}"#.to_owned()),
        (Subscription::AllMids { dex: coin("xyz") }, r#"{"type":"allMids","dex":"xyz"}"#.to_owned()),
        (Subscription::Notification { user: a }, format!(r#"{{"type":"notification","user":"{FIRST}"}}"#)),
        (Subscription::WebData3 { user: a }, format!(r#"{{"type":"webData3","user":"{FIRST}"}}"#)),
        (
            Subscription::TwapStates { user: a, dex: String::new() },
            format!(r#"{{"type":"twapStates","user":"{FIRST}","dex":""}}"#),
        ),
        (
            Subscription::ClearinghouseState { user: a, dex: String::new() },
            format!(r#"{{"type":"clearinghouseState","user":"{FIRST}","dex":""}}"#),
        ),
        (
            Subscription::OpenOrders { user: a, dex: String::new() },
            format!(r#"{{"type":"openOrders","user":"{FIRST}","dex":""}}"#),
        ),
        (
            Subscription::Candle { coin: coin("BTC"), interval: Interval::OneMinute },
            r#"{"type":"candle","coin":"BTC","interval":"1m"}"#.to_owned(),
        ),
        (
            Subscription::Candle { coin: coin("ETH"), interval: Interval::OneMonth },
            r#"{"type":"candle","coin":"ETH","interval":"1M"}"#.to_owned(),
        ),
        (book("BTC"), r#"{"type":"l2Book","coin":"BTC"}"#.to_owned()),
        (book("#14720"), r##"{"type":"l2Book","coin":"#14720"}"##.to_owned()),
        (
            Subscription::L2Book { coin: coin("ETH"), n_sig_figs: Some(5), mantissa: Some(2), fast: false },
            r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":2}"#.to_owned(),
        ),
        (
            Subscription::L2Book { coin: coin("SOL"), n_sig_figs: None, mantissa: None, fast: true },
            r#"{"type":"l2Book","coin":"SOL","fast":true}"#.to_owned(),
        ),
        (Subscription::Trades { coin: coin("BTC") }, r#"{"type":"trades","coin":"BTC"}"#.to_owned()),
        (Subscription::Trades { coin: coin("@107") }, r#"{"type":"trades","coin":"@107"}"#.to_owned()),
        (Subscription::OrderUpdates { user: a }, format!(r#"{{"type":"orderUpdates","user":"{FIRST}"}}"#)),
        (Subscription::UserEvents { user: a }, format!(r#"{{"type":"userEvents","user":"{FIRST}"}}"#)),
        (
            Subscription::UserFills { user: b, aggregate_by_time: false },
            format!(r#"{{"type":"userFills","user":"{SECOND}"}}"#),
        ),
        (
            Subscription::UserFills { user: b, aggregate_by_time: true },
            format!(r#"{{"type":"userFills","user":"{SECOND}","aggregateByTime":true}}"#),
        ),
        (Subscription::UserFundings { user: a }, format!(r#"{{"type":"userFundings","user":"{FIRST}"}}"#)),
        (
            Subscription::UserNonFundingLedgerUpdates { user: a },
            format!(r#"{{"type":"userNonFundingLedgerUpdates","user":"{FIRST}"}}"#),
        ),
        (Subscription::ActiveAssetCtx { coin: coin("BTC") }, r#"{"type":"activeAssetCtx","coin":"BTC"}"#.to_owned()),
        (Subscription::ActiveAssetCtx { coin: coin("@107") }, r#"{"type":"activeAssetCtx","coin":"@107"}"#.to_owned()),
        (
            Subscription::ActiveAssetData { user: a, coin: coin("BTC") },
            format!(r#"{{"type":"activeAssetData","user":"{FIRST}","coin":"BTC"}}"#),
        ),
        (Subscription::UserTwapSliceFills { user: a }, format!(r#"{{"type":"userTwapSliceFills","user":"{FIRST}"}}"#)),
        (Subscription::UserTwapHistory { user: a }, format!(r#"{{"type":"userTwapHistory","user":"{FIRST}"}}"#)),
        (Subscription::Bbo { coin: coin("BTC") }, r#"{"type":"bbo","coin":"BTC"}"#.to_owned()),
        (
            Subscription::SpotState { user: a, ignore_portfolio_margin: false },
            format!(r#"{{"type":"spotState","user":"{FIRST}"}}"#),
        ),
        (
            Subscription::SpotState { user: b, ignore_portfolio_margin: true },
            format!(r#"{{"type":"spotState","user":"{SECOND}","ignorePortfolioMargin":true}}"#),
        ),
        (
            Subscription::AllDexsClearinghouseState { user: a },
            format!(r#"{{"type":"allDexsClearinghouseState","user":"{FIRST}"}}"#),
        ),
        (Subscription::AllDexsAssetCtxs, r#"{"type":"allDexsAssetCtxs"}"#.to_owned()),
        (Subscription::OutcomeMetaUpdates, r#"{"type":"outcomeMetaUpdates"}"#.to_owned()),
        (Subscription::FastAssetCtxs, r#"{"type":"fastAssetCtxs"}"#.to_owned()),
        (Subscription::AssetCtxs { dex: coin("xyz") }, r#"{"type":"assetCtxs","dex":"xyz"}"#.to_owned()),
        (Subscription::SpotAssetCtxs, r#"{"type":"spotAssetCtxs"}"#.to_owned()),
        (
            Subscription::UserHistoricalOrders { user: a },
            format!(r#"{{"type":"userHistoricalOrders","user":"{FIRST}"}}"#),
        ),
    ]
}
