//! What readers need kept live, counted: the subscriptions each interest asks for, kept while any
//! interest needs them, and refused whole past the venue's limits.

use crate::client::Network;
use crate::live::store::fixtures::{FIRST, HOLDER, SECOND, address, frame, portfolio_frame};
use crate::live::store::{Changes, Interest, Store};
use crate::live::{BadSubscription, Frame, Limits, Refused, Subscription};
use crate::markets::Interval;
use crate::utils::Address;

fn store() -> Store {
    Store::new(Network::Mainnet)
}

/// A made-up account, `0x00..0n`.
fn someone(n: u8) -> Address {
    let mut bytes = [0u8; 20];
    bytes[19] = n;
    Address::from_bytes(bytes)
}

fn book(coin: &str) -> Interest {
    Interest::Book { coin: coin.to_owned() }
}

fn l2(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}

/// A subscription the venue acknowledged, read back from its echo.
fn acked(echo: Frame) -> Subscription {
    let Frame::Ack(ack) = echo else { panic!("an acknowledgement") };
    ack.subscription().unwrap()
}

fn subscribing(subscriptions: &[Subscription]) -> Changes {
    Changes { unsubscribe: Vec::new(), subscribe: subscriptions.to_vec() }
}

fn unsubscribing(subscriptions: &[Subscription]) -> Changes {
    Changes { unsubscribe: subscriptions.to_vec(), subscribe: Vec::new() }
}

/// What each interest asks for is what the venue acknowledged on 2026-10-08, echo for echo:
/// a full book (no grouping, not fast), a candle, the mids of the first dex and of `xyz`, and
/// fills not aggregated. A page's interests ask in the order they were taken, and are told once.
#[test]
fn each_interest_asks_for_what_the_venue_acknowledged_in_the_order_taken() {
    let first = address(FIRST);
    let cases = [
        (book("BTC"), "ack.l2Book"),
        (Interest::Candles { coin: "BTC".to_owned(), interval: Interval::OneMinute }, "ack.candle"),
        (Interest::Mids { dex: String::new() }, "ack.allMids"),
        (Interest::Mids { dex: "xyz".to_owned() }, "ack.allMids.xyz"),
        (Interest::Fills { user: first }, "ack.userFills"),
    ];
    for (interest, echo) in cases {
        assert_eq!(interest.subscriptions(), [acked(frame(echo))], "{interest:?}");
    }
    let state = Interest::State { user: first }.subscriptions();
    assert_eq!(state[1], acked(frame("ack.spotState")), "spot with portfolio margin read, not ignored");
    let state = Interest::State { user: address(HOLDER) }.subscriptions();
    assert_eq!(state[0], acked(portfolio_frame("ack.allDexsClearinghouseState")));
    assert_eq!(Interest::PerpContexts.subscriptions(), [Subscription::AllDexsAssetCtxs]);
    assert_eq!(Interest::SpotContexts.subscriptions(), [Subscription::SpotAssetCtxs]);

    let mut store = store();
    let coin = "xyz:NVDA";
    let _book = store.register(book(coin)).unwrap();
    let _trades = store.register(Interest::Trades { coin: coin.to_owned() }).unwrap();
    let _candles =
        store.register(Interest::Candles { coin: coin.to_owned(), interval: Interval::FifteenMinutes }).unwrap();
    let user = someone(0x22);
    let _state = store.register(Interest::State { user }).unwrap();
    let _main = store.register(Interest::Orders { user, dex: String::new() }).unwrap();
    let _xyz = store.register(Interest::Orders { user, dex: "xyz".to_owned() }).unwrap();
    let expected = [
        l2(coin),
        Subscription::Trades { coin: coin.to_owned() },
        Subscription::Candle { coin: coin.to_owned(), interval: Interval::FifteenMinutes },
        Subscription::AllDexsClearinghouseState { user },
        Subscription::SpotState { user, ignore_portfolio_margin: false },
        Subscription::WebData3 { user },
        Subscription::OpenOrders { user, dex: String::new() },
        Subscription::OpenOrders { user, dex: "xyz".to_owned() },
    ];
    assert_eq!(store.wanted(), expected);
    assert_eq!(store.changes(), subscribing(&expected));
    assert!(store.changes().is_empty(), "told once");
    assert_eq!(store.followed(), [user]);
}

/// Released or dropped, a handle ends its interest. What one interest needs and another does
/// not goes; what both need stays; letting go and taking hold again in between sends nothing.
#[test]
fn a_subscription_stays_while_any_interest_needs_it() {
    let mut store = store();
    let one = store.register(book("BTC")).unwrap();
    let two = store.register(book("BTC")).unwrap();
    assert_eq!(one.interest(), &book("BTC"));
    assert_eq!(store.changes(), subscribing(&[l2("BTC")]), "subscribed once");
    store.release(one);
    assert!(store.changes().is_empty(), "the other page still reads it");
    assert_eq!(store.wanted(), [l2("BTC")]);
    drop(two);
    assert_eq!(store.changes(), unsubscribing(&[l2("BTC")]));
    assert!(store.wanted().is_empty());

    let again = store.register(book("SOL")).unwrap();
    let _ = store.changes();
    drop(again);
    let _again = store.register(book("SOL")).unwrap();
    assert!(store.changes().is_empty(), "let go and taken again between two changes");

    let user = someone(7);
    let state = store.register(Interest::State { user }).unwrap();
    let _fills = store.register(Interest::Fills { user }).unwrap();
    let _ = store.changes();
    store.release(state);
    assert_eq!(
        store.changes(),
        unsubscribing(&[
            Subscription::AllDexsClearinghouseState { user },
            Subscription::SpotState { user, ignore_portfolio_margin: false },
            Subscription::WebData3 { user },
        ])
    );
    assert_eq!(store.followed(), [user], "still followed for its fills");
}

#[test]
fn followed_users_come_in_the_order_first_wanted_and_go_with_their_last_interest() {
    let mut store = store();
    let (first, second) = (address(FIRST), address(SECOND));
    let a = store.register(Interest::Fills { user: second }).unwrap();
    let b = store.register(Interest::State { user: first }).unwrap();
    let c = store.register(Interest::Ledger { user: second }).unwrap();
    let _d = store.register(book("BTC")).unwrap();
    let _e = store.register(Interest::Fundings { user: first }).unwrap();
    assert_eq!(store.followed(), [second, first]);
    store.release(a);
    assert_eq!(store.followed(), [second, first], "its ledger still follows it");
    store.release(c);
    store.release(b);
    assert_eq!(store.followed(), [first], "followed for its fundings");
}

/// The venue allows an IP ten users across its user subscriptions (`MAX_USERS`); another
/// interest in an account already followed is not an eleventh. A store sharing the IP with
/// another session takes a part of its subscriptions: past it a new subscription is refused
/// whole, and one already wanted is shared and costs nothing. One the venue would not take (no
/// coin, which drops the socket) is never asked for.
#[test]
fn an_interest_past_the_limits_or_one_the_venue_would_not_take_is_refused_whole() {
    let mut store = store();
    let held: Vec<_> = (1..=10).map(|n| store.register(Interest::State { user: someone(n) }).unwrap()).collect();
    assert_eq!(store.register(Interest::Fills { user: someone(11) }).err(), Some(Refused::Users { limit: 10 }));
    assert!(store.register(Interest::Fills { user: someone(1) }).is_ok());
    assert_eq!(store.followed(), (1..=10).map(someone).collect::<Vec<_>>(), "the first kept first");
    drop(held);
    assert!(store.register(Interest::Fills { user: someone(11) }).is_ok(), "room once the others let go");

    let mut store = self::store().with_limits(Limits { subscriptions: 3, users: 10, posts: 100 });
    let btc = store.register(book("BTC")).unwrap();
    let _trades = store.register(Interest::Trades { coin: "BTC".to_owned() }).unwrap();
    assert_eq!(store.register(Interest::State { user: someone(1) }).err(), Some(Refused::Subscriptions { limit: 3 }));
    assert_eq!(store.wanted().len(), 2, "none of the three it needed is wanted");
    let _mids = store.register(Interest::Mids { dex: String::new() }).unwrap();
    assert_eq!(store.register(book("ETH")).err(), Some(Refused::Subscriptions { limit: 3 }));
    let shared = store.register(book("BTC")).unwrap();
    store.release(btc);
    assert!(store.register(book("ETH")).is_err(), "the other BTC page still holds it");
    store.release(shared);
    let _eth = store.register(book("ETH")).unwrap();

    assert_eq!(store.register(book("")).err(), Some(Refused::Bad(BadSubscription::EmptyCoin)));
    assert_eq!(store.wanted().len(), 3, "nothing of it is wanted");
}
