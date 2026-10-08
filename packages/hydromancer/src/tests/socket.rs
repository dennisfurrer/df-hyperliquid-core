//! The socket's frames. `l4_update_782007304.json` is Hydromancer's documented `l4BookUpdates`
//! frame, verbatim; `builder_fills.json` is a live `builderFills` frame (hip4-tui, 2026-09-18).
//! The control frames are the ones the WebSocket and session pages document. Subscribe frames are
//! checked against the documented requests, read back as JSON.

use serde_json::{Value as Read, json};

use super::*;
use crate::client::{Hydromancer, Key, SOCKET};

const L4: &str = include_str!("fixtures/l4_update_782007304.json");
const FILLS: &str = include_str!("fixtures/builder_fills.json");

const ALICE: &str = "0x742d35cc6634c0532925a3b844bc9e7595f7f2e2";
const CAROL: &str = "0x8f3c55aa1122c0532925a3b844bc9e7595f7f9a1";
const BUILDER: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn json(text: &str) -> Read {
    serde_json::from_str(text).unwrap()
}

#[test]
fn the_socket_takes_the_key_in_its_url_escaped_as_a_browser_escapes_it() {
    let client = Hydromancer::mainnet((), Key::new("hm_live/a+b=c&d?e").unwrap());
    assert_eq!(client.socket_url(), "wss://api.hydromancer.xyz/ws?token=hm_live%2Fa%2Bb%3Dc%26d%3Fe");
    let plain = Hydromancer::mainnet((), Key::new("Az09-_.!~*'()").unwrap());
    assert_eq!(plain.socket_url(), "wss://api.hydromancer.xyz/ws?token=Az09-_.!~*'()", "the unreserved set passes");
    assert!(!SOCKET.contains("token"), "the bare socket, the only form that is printed");
}

#[test]
fn the_l4_subscription_is_the_documented_request_with_merge_pinned() {
    let documented = json(r#"{"method":"subscribe","subscription":{"type":"l4BookUpdates","coins":["ETH","BTC"]}}"#);
    let ours = json(&subscribe_l4(&["ETH", "BTC"]));
    assert_eq!(ours["method"], documented["method"]);
    assert_eq!(ours["subscription"]["type"], documented["subscription"]["type"]);
    assert_eq!(ours["subscription"]["coins"], documented["subscription"]["coins"]);
    assert_eq!(ours["subscription"]["merge"], json!(false), "the page asks for merge to be set explicitly");
    assert_eq!(
        json(&unsubscribe_l4(&["ETH"])),
        json(r#"{"method":"unsubscribe","subscription":{"type":"l4BookUpdates","coins":["ETH"]}}"#),
        "merge is refused on an unsubscribe"
    );
}

#[test]
fn the_fills_subscription_is_the_documented_request() {
    assert_eq!(
        json(&subscribe_fills(address(BUILDER))),
        json(&format!(r#"{{"type":"subscribe","subscription":{{"type":"builderFills","builder":"{BUILDER}"}}}}"#))
    );
    assert_eq!(json(&unsubscribe_fills(address(BUILDER)))["type"], "unsubscribe");
}

#[test]
fn the_heartbeat_frames_are_the_documented_ones() {
    assert_eq!(json(PONG), json(r#"{"type":"pong"}"#));
    assert_eq!(json(PING), json(r#"{"type":"ping"}"#));
}

#[test]
fn the_documented_l4_frame_reads_as_one_block_of_four_diffs() {
    let Ok(Message::Block(block)) = read(L4) else { panic!("{:?}", read(L4)) };
    assert_eq!((block.height, block.time), (782_007_304, 1_704_067_200_000));
    assert_eq!(block.coins, Some(vec!["ETH".to_owned()]));
    assert_eq!(
        block.diffs,
        [
            Diff::New {
                coin: "ETH".to_owned(),
                order: Order {
                    oid: 217_148_811_876,
                    user: address(ALICE),
                    side: Side::Buy,
                    px: dec("3245.5"),
                    sz: dec("1.5")
                },
                insert_before: None,
            },
            Diff::New {
                coin: "ETH".to_owned(),
                order: Order {
                    oid: 217_148_811_877,
                    user: address(CAROL),
                    side: Side::Buy,
                    px: dec("3245.5"),
                    sz: dec("0.8")
                },
                insert_before: Some(217_148_811_876),
            },
            Diff::Update { coin: "ETH".to_owned(), oid: 217_148_811_800, sz: dec("2") },
            Diff::Remove { coin: "ETH".to_owned(), oid: 217_148_811_750 },
        ]
    );
}

#[test]
fn an_all_markets_frame_names_no_coins_and_an_empty_block_still_counts() {
    let text =
        r#"{"type":"l4BookUpdates","channel":"l4BookUpdates","seq":9,"data":{"height":5,"timestamp":6,"diffs":[]}}"#;
    let Ok(Message::Block(block)) = read(text) else { panic!() };
    assert_eq!((block.height, block.coins, block.diffs.len()), (5, None, 0));
    let routed_on_channel = r#"{"channel":"l4BookUpdates","data":{"height":7,"timestamp":8,"diffs":[]}}"#;
    assert!(matches!(read(routed_on_channel), Ok(Message::Block(block)) if block.height == 7));
}

#[test]
fn an_l4_frame_that_does_not_read_is_refused_whole() {
    let broken = [
        L4.replace("\"3245.50\"", "\"3245.5x\""),
        L4.replace(ALICE, "0xnope"),
        L4.replace("\"side\": \"B\"", "\"side\": \"S\""),
        L4.replace("\"type\": \"remove\"", "\"type\": \"teleport\""),
        L4.replace("\"height\": 782007304,", ""),
        L4.replace("\"oid\": 217148811800,", ""),
        r#"{"type":"l4BookUpdates"}"#.to_owned(),
    ];
    for text in &broken {
        assert!(read(text).is_err(), "{text}");
    }
    assert!(read("not json at all").is_err());
}

#[test]
fn the_control_frames_read_as_what_they_are() {
    assert_eq!(read(r#"{"type":"connected","clientId":"abc123-def456","sessionId":"789xyz"}"#), Ok(Message::Welcome));
    assert_eq!(
        read(r#"{"type":"reconnected","clientId":"c","sessionId":"789xyz","subscriptions":["l4BookUpdates"]}"#),
        Ok(Message::Welcome)
    );
    assert_eq!(read(r#"{ "type": "ping" }"#), Ok(Message::Ping));
    assert_eq!(
        read(r#"{"type":"subscriptionUpdate","operation":"subscribe","subscribed":["builderFills"],"failed":[]}"#),
        Ok(Message::Ack)
    );
    assert_eq!(
        read(r#"{"type":"error","message":"Invalid API key"}"#),
        Ok(Message::Refused("Invalid API key".to_owned()))
    );
    assert_eq!(read(r#"{"type":"replay","data":[],"hasMore":false}"#), Ok(Message::Other("replay".to_owned())));
}

#[test]
fn a_fills_frame_reads_as_each_fill_with_its_trader() {
    let Ok(Message::Fills(fills)) = read(FILLS) else { panic!("{:?}", read(FILLS)) };
    assert_eq!(fills.len(), 2);
    let first = &fills[0];
    assert_eq!(first.user, address("0x4ee1ece8c089c545d43661015ba8930908cdbd1e"));
    assert_eq!((first.coin.as_str(), first.px, first.sz, first.side), ("#36020", dec("0.49501"), dec("40"), Side::Buy));
    assert_eq!((first.time, first.tx_index, first.builder_fee), (1_789_686_702_602, 16, None));
    assert_eq!(first.builder, Some(address(BUILDER)), "the upper-case spelling is the same builder");
    assert_eq!(first.dir.as_deref(), Some("Buy"));
    let second = &fills[1];
    assert_eq!((second.side, second.builder_fee, second.tx_index), (Side::Sell, Some(dec("0.02")), 0));
}

#[test]
fn an_empty_fills_frame_carries_no_fill_and_a_broken_one_is_refused() {
    assert_eq!(read(r#"{"type":"builderFills","seq":2,"fills":[]}"#), Ok(Message::Fills(Vec::new())));
    assert!(read(&FILLS.replace("\"0.49501\"", "0.49501")).is_err(), "a price as a float is not read through one");
    assert!(read(&FILLS.replace("0x4ee1ece8c089c545d43661015ba8930908cdbd1e", "trader")).is_err());
}
