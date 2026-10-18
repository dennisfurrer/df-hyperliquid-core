use crate::client::Network;
use crate::live::store::{Changes, Interest, OtherNetwork, Store};
use crate::live::{Limits, Refused, Session, Subscription, subscribe, unsubscribe};

fn store() -> Store {
    Store::new(Network::Mainnet)
}

fn book(coin: &str) -> Interest {
    Interest::Book { coin: coin.to_owned() }
}

fn l2(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: false }
}

fn subscribing(subscriptions: &[Subscription]) -> Changes {
    Changes { unsubscribe: Vec::new(), subscribe: subscriptions.to_vec() }
}

/// The frames the session would send itself, unsubscribes first; a socket that opens later
/// subscribes to everything wanted.
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
