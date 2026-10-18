//! One connection's subscriptions and posts: what is wanted is sent on every socket, once each,
//! and a post waits for its answer by id and is never sent again.

use crate::client::Network;

use super::*;
use crate::live::fixtures::mainnet;
use crate::live::{Frame, Post, Subscription, post, subscribe, unsubscribe};

fn trades(coin: &str) -> Subscription {
    Subscription::Trades { coin: coin.to_owned() }
}

fn fast_book(coin: &str) -> Subscription {
    Subscription::L2Book { coin: coin.to_owned(), n_sig_figs: None, mantissa: None, fast: true }
}

/// A session names its own network's socket. Subscriptions wait while it is closed and all go out
/// in order on open, on every socket: the venue keeps nothing for a new one. While open a new one
/// goes out at once, and one already wanted does not (sent twice, the venue answers `Already
/// subscribed`). An unsubscribe sends the same object the subscribe did: one whose options
/// differ is refused (`error.unsubscribe.fast`).
#[test]
fn what_is_wanted_goes_out_on_every_socket_once_each() {
    for network in Network::ALL {
        let session = Session::new(network);
        assert_eq!((session.network(), session.url()), (network, network.ws_url()));
        assert!(!session.is_open());
    }
    let mut session = Session::new(Network::Mainnet);
    assert_eq!(session.subscribe(trades("BTC")), Ok(None));
    assert_eq!(session.subscribe(fast_book("SOL")), Ok(None));
    assert_eq!(session.subscribe(trades("BTC")), Ok(None), "wanted once");
    let all = vec![subscribe(&trades("BTC")), subscribe(&fast_book("SOL"))];
    assert_eq!(session.opened(1_000), all);
    assert!(session.is_open());
    let _ = session.closed(2_000, 0);
    assert!(!session.is_open());
    assert_eq!(session.opened(3_000), all);

    assert_eq!(session.subscribe(trades("ETH")), Ok(Some(subscribe(&trades("ETH")))));
    assert_eq!(session.subscribe(trades("ETH")), Ok(None));
    let stop = session.unsubscribe(&fast_book("SOL")).unwrap();
    assert_eq!(stop, unsubscribe(&fast_book("SOL")));
    assert!(stop.contains(r#""fast":true"#));
    assert_eq!(session.unsubscribe(&fast_book("SOL")), None, "no longer wanted");
    assert_eq!(session.wanted(), [trades("BTC"), trades("ETH")]);
    let _ = session.closed(10_000, 0);
    assert_eq!(session.unsubscribe(&trades("BTC")), None, "nothing to send while closed");
    assert_eq!(session.opened(20_000), [subscribe(&trades("ETH"))], "and it is not asked for again on open");
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
