use crate::client::Network;

use super::*;
use crate::live::fixtures::mainnet;
use crate::live::{Frame, PING};

fn open(at: u64) -> Session {
    let mut session = Session::new(Network::Mainnet);
    let _ = session.opened(at);
    session
}

#[test]
fn a_quiet_socket_is_pinged_after_thirty_seconds_and_its_pong_keeps_it() {
    // The venue closes a connection it has sent nothing to for 60 s (timeouts and heartbeats);
    // the ping goes out at 30 s.
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
    // 250 ms doubling to 20 s, plus up to a quarter more.
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
    // other subscriptions were acknowledged. Acks alone must not reset the backoff, or such a
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
