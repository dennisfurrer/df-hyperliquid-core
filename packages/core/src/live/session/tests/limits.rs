use crate::client::Network;
use crate::utils::Address;

use super::*;
use crate::live::{Post, Session, Subscription};

fn trades(coin: &str) -> Subscription {
    Subscription::Trades { coin: coin.to_owned() }
}

/// A made-up account, `0x00..0n`.
fn someone(n: u8) -> Address {
    let mut bytes = [0u8; 20];
    bytes[19] = n;
    Address::from_bytes(bytes)
}

fn open(at: u64) -> Session {
    let mut session = Session::new(Network::Mainnet);
    let _ = session.opened(at);
    session
}

/// An answer to post `id`, shaped as the venue's answer to a read of an unknown coin (`post.info.null`).
fn answer(id: u64) -> String {
    format!(
        r#"{{"channel":"post","data":{{"id":{id},"response":{{"type":"info","payload":{{"type":"l2Book","data":null}}}}}}}}"#
    )
}

#[test]
fn the_venue_s_limits_are_refused_before_anything_is_sent() {
    // The rate limits page, per IP: 1000 subscriptions, 10 unique users across user
    // subscriptions, 100 posts in flight.
    assert_eq!(Limits::VENUE, Limits { subscriptions: 1_000, users: 10, posts: 100 });
    assert_eq!(Limits::default(), Limits::VENUE);

    let mut session = open(0);
    for n in 0..1_000 {
        assert!(matches!(session.subscribe(trades(&format!("C{n}"))), Ok(Some(_))), "{n}");
    }
    assert_eq!(session.subscribe(trades("MORE")), Err(Refused::Subscriptions { limit: 1_000 }));
    assert_eq!(session.wanted().len(), 1_000);
    assert_eq!(session.subscribe(trades("C0")), Ok(None), "one already kept is no more");
    let _ = session.unsubscribe(&trades("C0"));
    assert!(matches!(session.subscribe(trades("MORE")), Ok(Some(_))), "room again");

    let mut session = open(0);
    for n in 1..=10 {
        assert!(session.subscribe(Subscription::UserFills { user: someone(n), aggregate_by_time: false }).is_ok());
        assert!(session.subscribe(Subscription::OrderUpdates { user: someone(n) }).is_ok());
    }
    assert_eq!(session.users(), 10);
    assert_eq!(session.subscribe(Subscription::UserEvents { user: someone(11) }), Err(Refused::Users { limit: 10 }));
    assert!(session.subscribe(Subscription::UserEvents { user: someone(3) }).is_ok(), "a user already followed");
    assert!(session.subscribe(trades("BTC")).is_ok(), "market data names no user");
    let _ = session.unsubscribe(&Subscription::UserFills { user: someone(1), aggregate_by_time: false });
    assert_eq!(session.subscribe(Subscription::UserEvents { user: someone(11) }), Err(Refused::Users { limit: 10 }));
    let _ = session.unsubscribe(&Subscription::OrderUpdates { user: someone(1) });
    assert_eq!(session.users(), 9);
    assert!(session.subscribe(Subscription::UserEvents { user: someone(11) }).is_ok(), "the slot came free");
}

#[test]
fn a_bad_subscription_is_refused_before_it_is_kept() {
    let mut session = open(0);
    assert_eq!(session.subscribe(trades("")), Err(Refused::Bad(BadSubscription::EmptyCoin)));
    let lumped = Subscription::L2Book { coin: "BTC".to_owned(), n_sig_figs: Some(4), mantissa: Some(2), fast: false };
    assert_eq!(session.subscribe(lumped), Err(Refused::Bad(BadSubscription::Mantissa(2))));
    assert!(session.wanted().is_empty());
}

#[test]
fn smaller_limits_leave_room_for_other_sockets_on_the_same_ip() {
    let mut session = Session::new(Network::Mainnet).with_limits(Limits { subscriptions: 2, users: 1, posts: 1 });
    let _ = session.opened(0);
    assert!(session.subscribe(Subscription::OrderUpdates { user: someone(1) }).is_ok());
    assert_eq!(session.subscribe(Subscription::OrderUpdates { user: someone(2) }), Err(Refused::Users { limit: 1 }));
    assert!(session.subscribe(trades("BTC")).is_ok());
    assert_eq!(session.subscribe(trades("ETH")), Err(Refused::Subscriptions { limit: 2 }));
    let read = Post::info(r#"{"type":"allMids"}"#).unwrap();
    assert!(session.post(&read).is_ok());
    assert_eq!(session.post(&read), Err(Refused::Posts { limit: 1 }));
}

#[test]
fn a_hundred_posts_in_flight_is_the_most() {
    let mut session = open(0);
    let read = Post::info(r#"{"type":"allMids"}"#).unwrap();
    for n in 1..=100 {
        assert_eq!(session.post(&read).map(|(id, _)| id), Ok(n));
    }
    assert_eq!(session.post(&read), Err(Refused::Posts { limit: 100 }));
    assert!(session.received(5, &answer(40)).is_ok());
    assert_eq!(session.post(&read).map(|(id, _)| id), Ok(101));
}
