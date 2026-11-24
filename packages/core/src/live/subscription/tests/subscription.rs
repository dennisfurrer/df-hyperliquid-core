//! What a subscription says of itself: its type, the user it follows, the channel its data comes
//! on, and whether the venue would take it.

use crate::utils::Json;

use super::*;
use crate::live::fixtures::{FIRST, compressed as compressed_frame, mainnet};
use crate::live::subscription::fixtures::{book, coin, sent, user};

/// The docs' subscriptions page on 2026-10-08, in its order, then the three
/// `@nktkas/hyperliquid` 0.33.2 adds and the venue acknowledged, then the three the page added by
/// 2026-11-24. Only user subscriptions name a
/// user, and each names the channel its data comes on: its type, but for `userEvents` and a spot
/// or outcome coin's `activeAssetCtx`.
#[test]
fn each_subscription_names_its_type_its_user_and_its_channel() {
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
        "l2",
        "pac",
        "sac",
    ];
    let mut kinds: Vec<&str> = sent().iter().map(|(subscription, _)| subscription.kind()).collect();
    kinds.dedup();
    assert_eq!(kinds, listed);
    for (subscription, text) in sent() {
        let named = Json::parse(&text).unwrap().get("user").and_then(Json::as_str).map(user);
        assert_eq!(subscription.user(), named, "{text}");
    }

    let a = user(FIRST);
    // Each pair: a subscription and a frame recorded for it, whose `channel` it must name.
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
    let compressed = [
        (Subscription::L2 { coin: coin("BTC"), n_sig_figs: None, mantissa: None }, "l2"),
        (Subscription::L2 { coin: coin("BTC"), n_sig_figs: None, mantissa: None }, "l2.diff"),
        (Subscription::Pac, "pac"),
        (Subscription::Sac, "sac"),
    ];
    for (subscription, name) in compressed {
        let channel =
            Json::parse(&compressed_frame(name)).unwrap().get("channel").and_then(Json::as_str).map(ToOwned::to_owned);
        assert_eq!(Some(subscription.channel()), channel.as_deref(), "{name}");
    }
    // A spot pair by name and a HIP-3 perp answer as their kinds do.
    assert_eq!(Subscription::ActiveAssetCtx { coin: coin("PURR/USDC") }.channel(), "activeSpotAssetCtx");
    assert_eq!(Subscription::ActiveAssetCtx { coin: coin("xyz:SP500") }.channel(), "activeAssetCtx");
}

/// `nSigFigs` is 2, 3, 4 or 5; `mantissa` is 1, 2 or 5 and only with `nSigFigs` 5.
#[test]
fn a_book_s_grouping_is_held_to_what_the_info_docs_allow_and_a_coin_is_never_empty() {
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
    // `l2` groups as `l2Book` does.
    let l2 = |n_sig_figs, mantissa| Subscription::L2 { coin: coin("BTC"), n_sig_figs, mantissa }.check();
    assert_eq!((l2(Some(3), None), l2(Some(5), Some(5))), (Ok(()), Ok(())));
    assert_eq!(
        (l2(Some(6), None), l2(Some(4), Some(2))),
        (Err(BadSubscription::SigFigs(6)), Err(BadSubscription::Mantissa(2)))
    );
    assert_eq!(
        Subscription::L2 { coin: String::new(), n_sig_figs: None, mantissa: None }.check(),
        Err(BadSubscription::EmptyCoin)
    );
    assert_eq!(Subscription::Trades { coin: String::new() }.check(), Err(BadSubscription::EmptyCoin));
    assert_eq!(
        Subscription::ActiveAssetData { user: user(FIRST), coin: String::new() }.check(),
        Err(BadSubscription::EmptyCoin)
    );
}
