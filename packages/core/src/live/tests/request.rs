//! The frames sent, as the docs write them and as the venue took them: ping, subscribe,
//! unsubscribe, and a post carrying an info read or a signed action.

use crate::utils::Json;

use super::*;
use crate::live::fixtures::mainnet;
use crate::live::frame::{Answer, Frame, decode};
use crate::live::subscription::Subscription;

/// The timeouts-and-heartbeats page: `{ "method": "ping" }`, answered `{ "channel": "pong" }`.
/// A subscribe and an unsubscribe sent on mainnet on 2026-10-08 and acknowledged
/// (`ack.l2Book.sigFigs`, `ack.unsubscribe`), and the connecting page's example as wscat sent it.
#[test]
fn ping_subscribe_and_unsubscribe_are_the_frames_the_venue_takes() {
    assert_eq!(PING, Json::parse(r#"{ "method": "ping" }"#).unwrap().compact());
    assert_eq!(decode(&mainnet("pong")), Ok(Frame::Pong));
    let book = Subscription::L2Book { coin: "ETH".to_owned(), n_sig_figs: Some(5), mantissa: Some(2), fast: false };
    assert_eq!(
        subscribe(&book),
        r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":2}}"#
    );
    let trades = Subscription::Trades { coin: "@107".to_owned() };
    assert_eq!(unsubscribe(&trades), r#"{"method":"unsubscribe","subscription":{"type":"trades","coin":"@107"}}"#);
    let sol = Subscription::Trades { coin: "SOL".to_owned() };
    assert_eq!(
        subscribe(&sol),
        Json::parse(r#"{ "method": "subscribe", "subscription": { "type": "trades", "coin": "SOL" } }"#)
            .unwrap()
            .compact()
    );
}

/// The post-requests page's examples, with their ids: an info read, the body `/info` would take,
/// and a signed action, the whole `/exchange` body, keys and digits untouched. The read was sent
/// on mainnet as id 1 on 2026-10-08, and the venue's answer carries that id back. A post that is
/// not JSON is refused before it is sent.
#[test]
fn a_post_carries_an_info_read_or_an_action_as_written_in_the_docs_envelope() {
    let read = Post::info(r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}"#).unwrap();
    let docs = r#"{
      "method": "post",
      "id": 123,
      "request": {
        "type": "info",
        "payload": {
          "type": "l2Book",
          "coin": "ETH",
          "nSigFigs": 5,
          "mantissa": null
        }
      }
    }"#;
    assert_eq!(post(123, &read), Json::parse(docs).unwrap().compact());
    assert_eq!(
        post(1, &read),
        r#"{"method":"post","id":1,"request":{"type":"info","payload":{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}}}"#
    );
    let Ok(Frame::Post(reply)) = decode(&mainnet("post.info.l2Book")) else { panic!("a post answer") };
    assert_eq!(reply.id, 1);
    assert!(matches!(reply.answer, Answer::Info { ref kind, .. } if kind == "l2Book"));
    assert_eq!((read.kind(), read.payload().get("coin").and_then(Json::as_str)), ("info", Some("ETH")));

    let body = r#"{"action":{"type":"order","orders":[{"a":4,"b":true,"p":"1100","s":"0.2","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"},"nonce":1713825891591,"signature":{"r":"...","s":"...","v":"..."},"vaultAddress":"0x12...3"}"#;
    let docs = r#"{
      "method": "post",
      "id": 256,
      "request": {
        "type": "action",
        "payload": {
          "action": {
            "type": "order",
            "orders": [{"a": 4, "b": true, "p": "1100", "s": "0.2", "r": false, "t": {"limit": {"tif": "Gtc"}}}],
            "grouping": "na"
          },
          "nonce": 1713825891591,
          "signature": {
            "r": "...",
            "s": "...",
            "v": "..."
          },
          "vaultAddress": "0x12...3"
        }
      }
    }"#;
    let action = Post::action(body).unwrap();
    assert_eq!(action.kind(), "action");
    assert_eq!(post(256, &action), Json::parse(docs).unwrap().compact());
    assert!(Post::info("{\"type\":").is_err());
    assert!(Post::action("not json").is_err());
}
