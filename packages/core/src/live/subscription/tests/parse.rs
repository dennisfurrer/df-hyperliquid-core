use super::*;
use crate::live::fixtures::{FIRST, SECOND, mainnet};
use crate::live::subscription::fixtures::{book, coin, sent, user};
use crate::live::{Frame, decode};

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
