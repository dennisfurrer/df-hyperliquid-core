use crate::utils::Json;

use super::*;
use crate::live::fixtures::mainnet;
use crate::live::frame::{Answer, Frame, decode};
use crate::live::subscription::Subscription;

#[test]
fn ping_is_the_docs_heartbeat_and_the_venue_answers_it() {
    // The timeouts-and-heartbeats page: `{ "method": "ping" }`, answered `{ "channel": "pong" }`.
    assert_eq!(PING, Json::parse(r#"{ "method": "ping" }"#).unwrap().compact());
    assert_eq!(decode(&mainnet("pong")), Ok(Frame::Pong));
}

#[test]
fn subscribe_and_unsubscribe_wrap_the_subscription_as_the_venue_took_them() {
    // Both sent on mainnet on 2026-10-08 and acknowledged (`ack.l2Book.sigFigs`, `ack.unsubscribe`).
    let book = Subscription::L2Book { coin: "ETH".to_owned(), n_sig_figs: Some(5), mantissa: Some(2), fast: false };
    assert_eq!(
        subscribe(&book),
        r#"{"method":"subscribe","subscription":{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":2}}"#
    );
    let trades = Subscription::Trades { coin: "@107".to_owned() };
    assert_eq!(unsubscribe(&trades), r#"{"method":"unsubscribe","subscription":{"type":"trades","coin":"@107"}}"#);
    // The connecting page's example, as wscat sent it.
    let sol = Subscription::Trades { coin: "SOL".to_owned() };
    assert_eq!(
        subscribe(&sol),
        Json::parse(r#"{ "method": "subscribe", "subscription": { "type": "trades", "coin": "SOL" } }"#)
            .unwrap()
            .compact()
    );
}

#[test]
fn an_info_post_is_the_docs_envelope_and_was_answered_by_its_id() {
    let read = Post::info(r#"{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}"#).unwrap();
    // The post-requests page's example, with its id.
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
    // Sent on mainnet as id 1 on 2026-10-08; the venue's answer carries that id back.
    assert_eq!(
        post(1, &read),
        r#"{"method":"post","id":1,"request":{"type":"info","payload":{"type":"l2Book","coin":"ETH","nSigFigs":5,"mantissa":null}}}"#
    );
    let Ok(Frame::Post(reply)) = decode(&mainnet("post.info.l2Book")) else { panic!("a post answer") };
    assert_eq!(reply.id, 1);
    assert!(matches!(reply.answer, Answer::Info { ref kind, .. } if kind == "l2Book"));
}

#[test]
fn an_action_post_carries_the_exchange_body_as_written() {
    // The post-requests page's example: the `/exchange` body, keys and digits untouched.
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
}

#[test]
fn a_post_that_is_not_json_is_refused_before_it_is_sent() {
    assert!(Post::info("{\"type\":").is_err());
    assert!(Post::action("not json").is_err());
    assert_eq!(Post::info(r#"{"type":"meta"}"#).unwrap().kind(), "info");
}
