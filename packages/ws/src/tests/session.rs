use hl_core::{Address, Network};

use super::*;
use crate::{
    fixtures::mainnet,
    frame::Frame,
    request::{PING, Post, post, subscribe, unsubscribe},
    subscription::{BadSubscription, Subscription},
};

fn trades(coin: &str) -> Subscription {
    Subscription::Trades { coin: coin.to_owned() }
}

fn fast_book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: true }
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
fn a_session_names_its_own_network_s_socket() {
    for network in Network::ALL {
        let session = Session::new(network);
        assert_eq!((session.network(), session.url()), (network, network.ws_url()));
        assert!(!session.is_open());
    }
}

#[test]
fn subscriptions_wait_while_closed_and_all_go_out_in_order_on_open() {
    let mut session = Session::new(Network::Mainnet);
    assert_eq!(session.subscribe(trades("BTC")), Ok(None));
    assert_eq!(session.subscribe(fast_book("SOL")), Ok(None));
    assert_eq!(session.subscribe(trades("BTC")), Ok(None), "wanted once");
    let all = vec![subscribe(&trades("BTC")), subscribe(&fast_book("SOL"))];
    assert_eq!(session.opened(1_000), all);
    assert!(session.is_open());
    // Every reconnect resubscribes everything: the venue keeps nothing for a new socket.
    let _ = session.closed(2_000, 0);
    assert!(!session.is_open());
    assert_eq!(session.opened(3_000), all);
}

#[test]
fn while_open_a_new_subscription_goes_out_at_once_and_a_repeat_does_not() {
    let mut session = open(0);
    assert_eq!(session.subscribe(trades("BTC")), Ok(Some(subscribe(&trades("BTC")))));
    // Sent twice, the venue answers `Already subscribed` (`error.alreadySubscribed`).
    assert_eq!(session.subscribe(trades("BTC")), Ok(None));
    assert_eq!(session.wanted(), [trades("BTC")]);
}

#[test]
fn unsubscribing_sends_the_same_object_the_subscribe_did() {
    // An unsubscribe whose options differ from the subscribe's is refused (`error.unsubscribe.fast`).
    let mut session = open(0);
    let _ = session.subscribe(fast_book("SOL"));
    let _ = session.subscribe(trades("BTC"));
    let stop = session.unsubscribe(&fast_book("SOL")).unwrap();
    assert_eq!(stop, unsubscribe(&fast_book("SOL")));
    assert!(stop.contains(r#""fast":true"#));
    assert_eq!(session.unsubscribe(&fast_book("SOL")), None, "no longer wanted");
    assert_eq!(session.wanted(), [trades("BTC")]);
    let _ = session.closed(10, 0);
    assert_eq!(session.unsubscribe(&trades("BTC")), None, "nothing to send while closed");
    assert!(session.opened(20).is_empty(), "and nothing comes back on open");
}

#[test]
fn a_quiet_socket_is_pinged_after_thirty_seconds_and_its_pong_keeps_it() {
    // The venue closes a connection it has sent nothing to for 60 s (timeouts and heartbeats);
    // hip4-tui's D8 pings at 30 s.
    let mut session = open(1_000);
    assert_eq!(session.tick(30_999), Tick::Wait);
    assert_eq!(session.tick(31_000), Tick::Ping);
    assert_eq!(session.tick(31_500), Tick::Wait, "one ping, not one a tick");
    assert_eq!(session.received(31_200, &mainnet("pong")), Ok(Frame::Pong));
    assert_eq!(session.tick(61_199), Tick::Wait);
    assert_eq!(session.tick(61_200), Tick::Ping, "thirty seconds after the pong");
    assert_eq!(PING, r#"{"method":"ping"}"#);
}

#[test]
fn a_busy_socket_is_never_pinged() {
    let mut session = open(0);
    for at in (10_000..600_000).step_by(10_000) {
        assert!(session.received(at, &mainnet("bbo")).is_ok());
        assert_eq!(session.tick(at + 9_999), Tick::Wait, "{at}");
    }
}

#[test]
fn sixty_seconds_of_nothing_pong_included_is_a_dead_socket() {
    let mut session = open(0);
    assert_eq!(session.tick(30_000), Tick::Ping);
    assert_eq!(session.tick(59_999), Tick::Wait);
    assert_eq!(session.tick(60_000), Tick::Reconnect);
    assert_eq!(session.tick(90_000), Tick::Reconnect, "until the socket is closed");
    let _ = session.closed(90_001, 0);
    assert_eq!(session.tick(200_000), Tick::Wait, "a closed socket waits for its reconnect");

    // Anything that arrives counts, even what does not read.
    let mut session = open(0);
    assert!(session.received(50_000, "Websocket connection established.").is_err());
    assert_eq!(session.tick(109_999), Tick::Ping);
    assert_eq!(session.tick(109_999), Tick::Wait);
    assert_eq!(session.tick(110_000), Tick::Reconnect);
}

#[test]
fn due_says_when_a_tick_next_has_work() {
    let mut session = Session::new(Network::Mainnet);
    assert_eq!(session.due(), None, "closed");
    let _ = session.opened(1_000);
    assert_eq!(session.due(), Some(31_000));
    assert_eq!(session.tick(31_000), Tick::Ping);
    assert_eq!(session.due(), Some(61_000), "the silence limit and the next ping coincide");
    let _ = session.received(40_000, &mainnet("pong"));
    assert_eq!(session.due(), Some(70_000));
}

#[test]
fn reconnect_delays_double_from_a_quarter_second_to_twenty_and_never_stop() {
    // hip4-tui's ladder (`net/src/ws.rs`): 250 ms doubling to 20 s, plus up to a quarter more.
    let mut session = Session::new(Network::Testnet);
    let plain: Vec<u64> = (0..10).map(|at| session.closed(at, 0).after_ms).collect();
    assert_eq!(plain, [250, 500, 1_000, 2_000, 4_000, 8_000, 16_000, 20_000, 20_000, 20_000]);

    for random in [1, 62, 63, 64, 0xdead_beef, u64::MAX] {
        let first = Session::new(Network::Testnet).closed(0, random).after_ms;
        assert_eq!(first, 250 + random % 63, "{random}");
    }

    // Ten thousand failures in a row: always another try, never past 25 s.
    let mut session = Session::new(Network::Mainnet);
    let mut random = 0x9e37_79b9_7f4a_7c15_u64;
    for attempt in 0..10_000u64 {
        random = random.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let after = session.closed(attempt, random).after_ms;
        assert!((250..=25_000).contains(&after), "{attempt}: {after}");
        if attempt >= 7 {
            assert!(after >= 20_000, "{attempt}: {after}");
        }
    }
}

#[test]
fn a_socket_that_held_starts_the_ladder_over_and_one_dropped_at_once_does_not() {
    // On 2026-10-08 a `bbo` for an unknown coin got the socket dropped within a second, after the
    // other subscriptions were acknowledged. Acks alone must not reset the ladder, or such a
    // subscription would reconnect four times a second.
    let mut session = Session::new(Network::Mainnet);
    let mut at = 0;
    for expected in [250, 500, 1_000, 2_000] {
        let _ = session.opened(at);
        assert!(session.received(at + 200, &mainnet("ack.allMids")).is_ok());
        assert_eq!(session.closed(at + 600, 0).after_ms, expected);
        at += 10_000;
    }
    // Thirty seconds open and talking: the socket worked, and the next failure starts over.
    let _ = session.opened(at);
    let _ = session.received(at + 29_000, &mainnet("bbo"));
    assert_eq!(session.closed(at + 30_000, 0).after_ms, 250);
    assert_eq!(session.closed(at + 30_500, 0).after_ms, 500, "a failed connect climbs again");
    // Open long but silent proves nothing.
    let _ = session.opened(at + 40_000);
    assert_eq!(session.closed(at + 95_000, 0).after_ms, 1_000);
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
fn posts_wait_for_their_answer_by_id_and_a_dropped_socket_reports_them_unanswered() {
    let read = Post::info(r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}"#).unwrap();
    let mut session = Session::new(Network::Mainnet);
    assert_eq!(session.post(&read), Err(Refused::Closed), "never held to send later");
    let _ = session.opened(0);
    // Ids count from one, as the recording's did; the venue answered id 1 and refused id 2.
    let (one, text) = session.post(&read).unwrap();
    assert_eq!((one, text.as_str()), (1, post(1, &read).as_str()));
    let (two, _) = session.post(&Post::info(r#"{"type":"noSuchRead"}"#).unwrap()).unwrap();
    let (three, _) = session.post(&read).unwrap();
    assert_eq!((two, three), (2, 3));
    assert_eq!(session.waiting(), [1, 2, 3]);
    assert!(matches!(session.received(10, &mainnet("post.info.l2Book")), Ok(Frame::Post(ref reply)) if reply.id == 1));
    assert!(matches!(session.received(11, &mainnet("error.post")), Ok(Frame::Error(_))));
    assert_eq!(session.waiting(), [3]);
    // Unknown, not failed: an action may have landed, so it is reported and never resent.
    assert_eq!(session.closed(20, 0).unanswered, [3]);
    assert!(session.waiting().is_empty());
    let _ = session.opened(30);
    assert_eq!(session.post(&read).map(|(id, _)| id), Ok(4), "an id is never reused");
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
