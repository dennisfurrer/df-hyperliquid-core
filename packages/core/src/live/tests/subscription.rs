use crate::utils::{Address, Json};

use super::*;
use crate::live::fixtures::mainnet;
use crate::live::frame::{Frame, decode};

/// The two accounts the mainnet recording followed on 2026-10-08.
const FIRST: &str = "0x42f1b36fb856769e075dfbf25938464fb3c92c70";
const SECOND: &str = "0x78dce96236804d6958bf3c4598cf5efbc371629c";

fn user(text: &str) -> Address {
    text.parse().unwrap()
}

fn coin(text: &str) -> String {
    text.to_owned()
}

fn book(name: &str) -> Subscription {
    Subscription::L2Book { coin: coin(name), n_sig_figs: None, mantissa: None, fast: false }
}

/// Every subscription with the exact object sent for it on mainnet on 2026-10-08, each of which
/// the venue acknowledged. Keys are in the order the docs' subscription messages write them,
/// optional ones last and only when set, as the Python SDK sends them.
fn sent() -> Vec<(Subscription, String)> {
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

#[test]
fn each_subscription_is_written_as_it_was_sent_and_taken() {
    for (subscription, text) in sent() {
        assert_eq!(subscription.to_json().compact(), text);
        assert_eq!(subscription.to_string(), text);
        assert_eq!(subscription.check(), Ok(()), "{text}");
    }
}

#[test]
fn every_type_the_docs_list_is_covered_once() {
    // The docs' subscriptions page on 2026-10-08, in its order, then the three
    // @nktkas/hyperliquid 0.33.2 adds and the venue acknowledged.
    let listed = [
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
    let mut kinds: Vec<&str> = sent().iter().map(|(subscription, _)| subscription.kind()).collect();
    kinds.dedup();
    assert_eq!(kinds, listed);
}

#[test]
fn what_the_venue_echoes_reads_back_to_the_same_subscription() {
    // The venue's acknowledgements fill in what was left out (`nSigFigs: null`, `fast: false`,
    // `aggregateByTime: false`, `ignorePortfolioMargin: false`, `dex: ""`), lowercase the user
    // and put keys in their own order. Each still names the subscription that was sent.
    let (a, b) = (user(FIRST), user(SECOND));
    let cases = [
        ("ack.allMids", Subscription::AllMids { dex: String::new() }),
        ("ack.allMids.xyz", Subscription::AllMids { dex: coin("xyz") }),
        ("ack.l2Book", book("BTC")),
        (
            "ack.l2Book.sigFigs",
            Subscription::L2Book { coin: coin("ETH"), n_sig_figs: Some(5), mantissa: Some(2), fast: false },
        ),
        ("ack.l2Book.fast", Subscription::L2Book { coin: coin("SOL"), n_sig_figs: None, mantissa: None, fast: true }),
        ("ack.candle", Subscription::Candle { coin: coin("BTC"), interval: Interval::OneMinute }),
        ("ack.activeAssetData", Subscription::ActiveAssetData { user: a, coin: coin("BTC") }),
        ("ack.clearinghouseState", Subscription::ClearinghouseState { user: a, dex: String::new() }),
        ("ack.twapStates", Subscription::TwapStates { user: b, dex: String::new() }),
        ("ack.spotState", Subscription::SpotState { user: a, ignore_portfolio_margin: false }),
        ("ack.spotState.ignore", Subscription::SpotState { user: b, ignore_portfolio_margin: true }),
        ("ack.userFills", Subscription::UserFills { user: a, aggregate_by_time: false }),
        ("ack.userFills.aggregate", Subscription::UserFills { user: b, aggregate_by_time: true }),
        // Sent with the address in capitals; echoed lowercase.
        ("ack.userEvents", Subscription::UserEvents { user: b }),
        ("ack.unsubscribe", Subscription::Trades { coin: coin("@107") }),
        ("ack.unsubscribe.candle", Subscription::Candle { coin: coin("ETH"), interval: Interval::OneMonth }),
    ];
    for (name, expected) in cases {
        let Ok(Frame::Ack(ack)) = decode(&mainnet(name)) else { panic!("{name} is an ack") };
        assert_eq!(ack.subscription(), Ok(expected), "{name}");
    }
}

#[test]
fn a_subscription_reads_from_its_own_object_whatever_the_key_order() {
    for (subscription, text) in sent() {
        assert_eq!(Subscription::parse(&Json::parse(&text).unwrap()), Ok(subscription.clone()), "{text}");
    }
    let shuffled = Json::parse(r#"{"interval":"1M","coin":"ETH","type":"candle"}"#).unwrap();
    assert_eq!(
        Subscription::parse(&shuffled),
        Ok(Subscription::Candle { coin: coin("ETH"), interval: Interval::OneMonth })
    );
}

#[test]
fn what_is_not_a_subscription_is_refused_by_name() {
    let read = |text: &str| Subscription::parse(&Json::parse(text).unwrap());
    // The venue refused both of these on 2026-10-08 (`error.webData2`, `error.interval`).
    assert_eq!(
        read(r#"{"type":"webData2","user":"0x42f1b36fb856769e075dfbf25938464fb3c92c70"}"#),
        Err(BadSubscription::UnknownType("webData2".to_owned()))
    );
    assert!(matches!(read(r#"{"type":"candle","coin":"ETH","interval":"7m"}"#), Err(BadSubscription::Wrong { .. })));
    // And this one, which names no coin (`error.invalidSubscription`).
    assert_eq!(read(r#"{"type":"l2Book"}"#), Err(BadSubscription::Missing { kind: "l2Book", field: "coin" }));
    assert!(matches!(read(r#"{"type":"userFills","user":"0xdeadbeef"}"#), Err(BadSubscription::Wrong { .. })));
    assert_eq!(read(r#"["allMids"]"#), Err(BadSubscription::NotAnObject));
    assert_eq!(read(r#"{"coin":"BTC"}"#), Err(BadSubscription::Missing { kind: "subscription", field: "type" }));
}

#[test]
fn every_interval_the_docs_list_reads_and_writes_back() {
    let listed = ["1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h", "1d", "3d", "1w", "1M"];
    assert_eq!(Interval::ALL.map(Interval::wire), listed);
    for text in listed {
        assert_eq!(Interval::parse(text).map(Interval::wire), Some(text));
    }
    for refused in ["7m", "1M ", "1H", "", "60m"] {
        assert_eq!(Interval::parse(refused), None, "{refused:?}");
    }
}

#[test]
fn book_grouping_is_held_to_what_the_info_docs_allow() {
    // `nSigFigs` is 2, 3, 4 or 5; `mantissa` is 1, 2 or 5 and only with `nSigFigs` 5.
    let grouped =
        |n_sig_figs, mantissa| Subscription::L2Book { coin: coin("BTC"), n_sig_figs, mantissa, fast: false }.check();
    for figures in 2..=5 {
        assert_eq!(grouped(Some(figures), None), Ok(()), "{figures}");
    }
    for mantissa in [1, 2, 5] {
        assert_eq!(grouped(Some(5), Some(mantissa)), Ok(()), "{mantissa}");
    }
    assert_eq!(grouped(Some(1), None), Err(BadSubscription::SigFigs(1)));
    assert_eq!(grouped(Some(6), None), Err(BadSubscription::SigFigs(6)));
    assert_eq!(grouped(Some(4), Some(2)), Err(BadSubscription::Mantissa(2)));
    assert_eq!(grouped(None, Some(2)), Err(BadSubscription::Mantissa(2)));
    assert_eq!(grouped(Some(5), Some(3)), Err(BadSubscription::Mantissa(3)));
    assert_eq!(Subscription::Trades { coin: String::new() }.check(), Err(BadSubscription::EmptyCoin));
    assert_eq!(
        Subscription::ActiveAssetData { user: user(FIRST), coin: String::new() }.check(),
        Err(BadSubscription::EmptyCoin)
    );
}

#[test]
fn each_subscription_names_the_channel_its_data_comes_on() {
    // Each pair: a subscription and a frame recorded for it, whose `channel` it must name.
    let a = user(FIRST);
    let pairs = [
        (Subscription::ActiveAssetCtx { coin: coin("BTC") }, "activeAssetCtx"),
        (Subscription::ActiveAssetCtx { coin: coin("@107") }, "activeSpotAssetCtx"),
        (Subscription::ActiveAssetCtx { coin: coin("#14720") }, "activeSpotAssetCtx.outcome"),
        (Subscription::UserEvents { user: a }, "user.fills"),
        (Subscription::Trades { coin: coin("BTC") }, "trades"),
        (book("BTC"), "l2Book"),
        (Subscription::AllMids { dex: String::new() }, "allMids"),
        (Subscription::UserFills { user: a, aggregate_by_time: false }, "userFills"),
        (Subscription::ClearinghouseState { user: a, dex: String::new() }, "clearinghouseState"),
        (Subscription::SpotState { user: a, ignore_portfolio_margin: false }, "spotState"),
    ];
    for (subscription, name) in pairs {
        let channel = Json::parse(&mainnet(name)).unwrap().get("channel").and_then(Json::as_str).map(ToOwned::to_owned);
        assert_eq!(Some(subscription.channel()), channel.as_deref(), "{name}");
    }
    // A spot pair by name and a HIP-3 perp answer as their kinds do.
    assert_eq!(Subscription::ActiveAssetCtx { coin: coin("PURR/USDC") }.channel(), "activeSpotAssetCtx");
    assert_eq!(Subscription::ActiveAssetCtx { coin: coin("xyz:SP500") }.channel(), "activeAssetCtx");
}

#[test]
fn only_user_subscriptions_name_a_user() {
    for (subscription, text) in sent() {
        let named = Json::parse(&text).unwrap().get("user").and_then(Json::as_str).map(user);
        assert_eq!(subscription.user(), named, "{text}");
    }
}
