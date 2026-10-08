use hl_core::{Address, Network};
use hl_ws::{BadSubscription, Frame, Interval, Limits, Refused, Session, Subscription, subscribe, unsubscribe};

use crate::{
    Changes, Interest, OtherNetwork, Store,
    fixtures::{FIRST, SECOND, TUI, address, frame, tui_frame},
};

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

/// A subscription the venue acknowledged, read back from its echo as hl-ws reads one.
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
/// fills not aggregated.
#[test]
fn each_interest_asks_for_what_the_venue_acknowledged() {
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
    let state = Interest::State { user: address(TUI) }.subscriptions();
    assert_eq!(state[0], acked(tui_frame("ack.allDexsClearinghouseState")));
}

/// hip4-tui's market on screen: its book, its prints and its candles, on the same socket.
#[test]
fn a_market_page_asks_for_its_book_prints_and_candles_in_order() {
    let mut store = store();
    let coin = "xyz:NVDA";
    let _book = store.register(book(coin)).unwrap();
    let _trades = store.register(Interest::Trades { coin: coin.to_owned() }).unwrap();
    let _candles =
        store.register(Interest::Candles { coin: coin.to_owned(), interval: Interval::FifteenMinutes }).unwrap();
    let expected = [
        l2(coin),
        Subscription::Trades { coin: coin.to_owned() },
        Subscription::Candle { coin: coin.to_owned(), interval: Interval::FifteenMinutes },
    ];
    assert_eq!(store.wanted(), expected);
    assert_eq!(store.changes(), subscribing(&expected));
    assert!(store.changes().is_empty(), "told once");
}

/// hip4-tui streams each followed account's every dex, spot balances and mode, and its resting
/// orders dex by dex.
#[test]
fn an_account_asks_for_its_state_and_its_orders_dex_by_dex() {
    let mut store = store();
    let user = someone(0x22);
    let _state = store.register(Interest::State { user }).unwrap();
    let _main = store.register(Interest::Orders { user, dex: String::new() }).unwrap();
    let _xyz = store.register(Interest::Orders { user, dex: "xyz".to_owned() }).unwrap();
    assert_eq!(
        store.wanted(),
        [
            Subscription::AllDexsClearinghouseState { user },
            Subscription::SpotState { user, ignore_portfolio_margin: false },
            Subscription::WebData3 { user },
            Subscription::OpenOrders { user, dex: String::new() },
            Subscription::OpenOrders { user, dex: "xyz".to_owned() },
        ]
    );
    assert_eq!(store.followed(), [user]);
}

/// Two pages on one book share one subscription, which goes when the last lets go.
#[test]
fn a_subscription_stays_while_any_interest_needs_it() {
    let mut store = store();
    let one = store.register(book("BTC")).unwrap();
    let two = store.register(book("BTC")).unwrap();
    assert_eq!(store.changes(), subscribing(&[l2("BTC")]), "subscribed once");
    store.release(one);
    assert!(store.changes().is_empty(), "the other page still reads it");
    assert_eq!(store.wanted(), [l2("BTC")]);
    store.release(two);
    assert_eq!(store.changes(), unsubscribing(&[l2("BTC")]));
    assert!(store.wanted().is_empty());
}

#[test]
fn dropping_a_handle_ends_its_interest() {
    let mut store = store();
    let handle = store.register(book("ETH")).unwrap();
    assert_eq!(handle.interest(), &book("ETH"));
    let _ = store.changes();
    drop(handle);
    assert_eq!(store.changes(), unsubscribing(&[l2("ETH")]));
}

/// atlas: a panel moved in the grid finds its market live. Letting go and taking hold again
/// before the socket is next told sends nothing.
#[test]
fn letting_go_and_taking_hold_again_in_between_sends_nothing() {
    let mut store = store();
    let handle = store.register(book("SOL")).unwrap();
    let _ = store.changes();
    drop(handle);
    let _again = store.register(book("SOL")).unwrap();
    assert!(store.changes().is_empty());
}

/// What one interest needs and another does not goes; what both need stays.
#[test]
fn an_account_s_state_and_its_fills_are_counted_apart() {
    let mut store = store();
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

/// The venue allows an IP ten users across its user subscriptions (hl-ws's `MAX_USERS`;
/// hip4-tui follows at most ten). An eleventh is refused; another interest in one already
/// followed is not.
#[test]
fn an_eleventh_user_is_refused() {
    let mut store = store();
    let held: Vec<_> = (1..=10).map(|n| store.register(Interest::State { user: someone(n) }).unwrap()).collect();
    assert_eq!(store.register(Interest::Fills { user: someone(11) }).err(), Some(Refused::Users { limit: 10 }));
    assert!(store.register(Interest::Fills { user: someone(1) }).is_ok());
    assert_eq!(store.followed(), (1..=10).map(someone).collect::<Vec<_>>(), "the first kept first");
    drop(held);
    assert!(store.register(Interest::Fills { user: someone(11) }).is_ok(), "room once the others let go");
}

/// A store sharing the IP with another session takes a part of its allowance. Past it, a new
/// subscription is refused whole; one already wanted is shared, and costs nothing.
#[test]
fn past_the_subscription_limit_an_interest_is_refused_whole() {
    let mut store = store().with_limits(Limits { subscriptions: 3, users: 10, posts: 100 });
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
    assert!(store.register(book("ETH")).is_ok());
}

/// The venue drops the socket for some subscriptions it cannot serve; one with no coin is never
/// asked for.
#[test]
fn an_interest_the_venue_would_not_take_is_refused() {
    let mut store = store();
    assert_eq!(store.register(book("")).err(), Some(Refused::Bad(BadSubscription::EmptyCoin)));
    assert!(store.wanted().is_empty());
}

/// Into an hl-ws session: the frames it would send itself, unsubscribes first; and a socket
/// that opens later subscribes to everything wanted.
#[test]
fn sync_gives_the_session_s_own_frames() {
    let mut store = store();
    let mut session = Session::new(Network::Mainnet);
    let btc = store.register(book("BTC")).unwrap();
    let synced = store.sync(&mut session).unwrap();
    assert!(synced.frames.is_empty(), "nothing is sent on a closed socket");
    assert_eq!(session.wanted(), [l2("BTC")]);
    assert_eq!(session.opened(1_000), [subscribe(&l2("BTC"))]);

    let _eth = store.register(book("ETH")).unwrap();
    store.release(btc);
    let synced = store.sync(&mut session).unwrap();
    assert_eq!(synced.frames, [unsubscribe(&l2("BTC")), subscribe(&l2("ETH"))]);
    assert!(synced.refused.is_empty());
    assert_eq!(session.wanted(), store.wanted());
}

/// A session that will not take one is told so in the result, and asked again next time.
#[test]
fn what_the_session_refuses_is_reported_and_asked_again() {
    let mut store = store();
    let mut session = Session::new(Network::Mainnet).with_limits(Limits { subscriptions: 1, users: 10, posts: 100 });
    let _ = session.opened(0);
    let _btc = store.register(book("BTC")).unwrap();
    let _eth = store.register(book("ETH")).unwrap();
    let synced = store.sync(&mut session).unwrap();
    assert_eq!(synced.frames, [subscribe(&l2("BTC"))]);
    assert_eq!(synced.refused, [(l2("ETH"), Refused::Subscriptions { limit: 1 })]);
    assert_eq!(store.sync(&mut session).unwrap().refused, [(l2("ETH"), Refused::Subscriptions { limit: 1 })]);
}

/// hip4-tui's D13: a session is one network. A store never feeds a session on the other one.
#[test]
fn a_store_never_feeds_a_session_on_the_other_network() {
    let mut store = store();
    let _btc = store.register(book("BTC")).unwrap();
    let mut testnet = Session::new(Network::Testnet);
    assert_eq!(
        store.sync(&mut testnet).err(),
        Some(OtherNetwork { store: Network::Mainnet, session: Network::Testnet })
    );
    assert!(testnet.wanted().is_empty());
    assert_eq!(store.changes(), subscribing(&[l2("BTC")]), "nothing was taken as told");
    assert_eq!(store.network(), Network::Mainnet);
}

#[test]
fn followed_users_come_in_the_order_first_wanted_and_go_with_their_last_interest() {
    let mut store = store();
    let (first, second) = (address(FIRST), address(SECOND));
    let a = store.register(Interest::Fills { user: second }).unwrap();
    let _b = store.register(Interest::State { user: first }).unwrap();
    let _c = store.register(Interest::Ledger { user: second }).unwrap();
    let _d = store.register(book("BTC")).unwrap();
    assert_eq!(store.followed(), [second, first]);
    store.release(a);
    assert_eq!(store.followed(), [second, first], "its ledger still follows it");
}
