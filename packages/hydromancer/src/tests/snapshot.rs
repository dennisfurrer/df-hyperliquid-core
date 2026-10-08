//! The `l4Book` frame. `l4_snapshot.bin` was packed by Python's msgpack and zstandard, not by
//! this crate, and `l4_snapshot.json` is that frame as Hydromancer's own Python SDK reads it; the
//! levels each side adds up to are the SDK's (`l4_books.json`). See `fixtures/README.md`.
//! Frames that must be refused are built here with hl-core's msgpack encoder and ruzstd's
//! encoder, each shaped to break one rule.

use hl_core::Value;
use ruzstd::encoding::{CompressionLevel, compress_to_vec};
use serde_json::Value as Read;

use super::*;
use crate::book::{Level, levels};

const FRAME: &[u8] = include_bytes!("fixtures/l4_snapshot.bin");
const SDK: &str = include_str!("fixtures/l4_snapshot.json");
const BOOKS: &str = include_str!("fixtures/l4_books.json");

const ALICE: &str = "0x742d35cc6634c0532925a3b844bc9e7595f7f2e2";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn sdk() -> Read {
    serde_json::from_str(SDK).unwrap()
}

/// One market's blob: msgpack `[coin, bids, asks]`, zstd around it.
fn blob(market: &Value) -> Vec<u8> {
    compress_to_vec(market.to_msgpack().as_slice(), CompressionLevel::Fastest)
}

fn order(user: &str, px: &str, sz: &str, oid: u64) -> Value {
    Value::array([
        Value::from(user.to_owned()),
        Value::from(px.to_owned()),
        Value::from(sz.to_owned()),
        Value::Uint(oid),
    ])
}

fn market(coin: &str, bids: Vec<Value>, asks: Vec<Value>) -> Value {
    Value::array([Value::from(coin.to_owned()), Value::Array(bids), Value::Array(asks)])
}

/// `[count][height][timestamp_ms]`, then each blob behind its length.
fn frame(count: u32, blobs: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&782_007_303_u64.to_le_bytes());
    out.extend_from_slice(&1_704_067_199_930_u64.to_le_bytes());
    for blob in blobs {
        out.extend_from_slice(&u32::try_from(blob.len()).unwrap().to_le_bytes());
        out.extend_from_slice(blob);
    }
    out
}

#[test]
fn the_frame_reads_as_the_provider_s_own_sdk_reads_it() {
    let read = decode(FRAME).unwrap();
    let sdk = sdk();
    assert_eq!(read.height, sdk["height"].as_u64().unwrap());
    assert_eq!(read.time, sdk["timestamp_ms"].as_u64().unwrap());
    let markets = sdk["markets"].as_array().unwrap();
    assert_eq!(read.markets.len(), markets.len());
    for (ours, theirs) in read.markets.iter().zip(markets) {
        assert_eq!(ours.coin, theirs["coin"].as_str().unwrap());
        for (side, ours, theirs) in [(Side::Bid, &ours.bids, &theirs["bids"]), (Side::Ask, &ours.asks, &theirs["asks"])]
        {
            let theirs = theirs.as_array().unwrap();
            assert_eq!(ours.len(), theirs.len(), "{} {side:?}", ours.len());
            for (order, sdk) in ours.iter().zip(theirs) {
                assert_eq!(order.oid, sdk["oid"].as_u64().unwrap());
                assert_eq!(order.user.to_hex(), sdk["user"].as_str().unwrap());
                assert_eq!(order.px, dec(sdk["px"].as_str().unwrap()));
                assert_eq!(order.sz, dec(sdk["sz"].as_str().unwrap()));
                assert_eq!(order.side, side);
            }
        }
    }
}

#[test]
fn a_blob_without_its_size_or_checksum_reads_too() {
    let read = decode(FRAME).unwrap();
    let btc = read.markets.iter().find(|market| market.coin == "BTC").unwrap();
    assert_eq!(btc.bids[0].px, dec("108708.4"));
    assert_eq!(btc.asks[0].user, "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap());
}

#[test]
fn each_side_adds_up_to_the_levels_the_sdk_holds() {
    let read = decode(FRAME).unwrap();
    let books: Read = serde_json::from_str(BOOKS).unwrap();
    for market in &read.markets {
        let sdk = &books["books"]["782007303"][&market.coin];
        for (side, orders, key) in [(Side::Bid, &market.bids, "bids"), (Side::Ask, &market.asks, "asks")] {
            let expected: Vec<Level> = sdk[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|level| Level {
                    px: dec(level[0].as_str().unwrap()),
                    sz: dec(level[1].as_str().unwrap()),
                    orders: usize::try_from(level[2].as_u64().unwrap()).unwrap(),
                })
                .collect();
            assert_eq!(levels(orders, side), expected, "{} {key}", market.coin);
        }
    }
}

#[test]
fn a_frame_cut_anywhere_is_refused_and_never_read_past() {
    for end in 0..FRAME.len() {
        assert!(decode(&FRAME[..end]).is_err(), "cut at {end} of {}", FRAME.len());
    }
}

#[test]
fn a_count_the_frame_does_not_hold_is_refused() {
    let eth = blob(&market("ETH", vec![order(ALICE, "1", "2", 3)], Vec::new()));
    assert!(decode(&frame(1, std::slice::from_ref(&eth))).is_ok());
    assert!(decode(&frame(2, std::slice::from_ref(&eth))).is_err(), "a count larger than the blobs");
    assert!(decode(&frame(u32::MAX, std::slice::from_ref(&eth))).is_err(), "nothing is reserved on its word");
    assert!(decode(&frame(0, std::slice::from_ref(&eth))).is_err(), "bytes after the last market");
    assert!(decode(&frame(0, &[])).unwrap().markets.is_empty(), "an empty frame is an empty book");
}

#[test]
fn a_blob_that_does_not_open_is_refused() {
    let mut damaged = FRAME.to_vec();
    // Inside the first blob's compressed bytes.
    damaged[20 + 4 + 40] ^= 0xff;
    assert!(decode(&damaged).is_err());
    assert!(decode(&frame(1, &[b"not zstd at all".to_vec()])).is_err());
}

#[test]
fn a_blob_whose_checksum_does_not_match_is_refused_though_it_reads() {
    let eth_len = usize::try_from(u32::from_le_bytes(FRAME[20..24].try_into().unwrap())).unwrap();
    let mut damaged = FRAME.to_vec();
    // The first blob's last four bytes are its content checksum; the bytes before them still open.
    damaged[20 + 4 + eth_len - 1] ^= 0x01;
    let refused = decode(&damaged).unwrap_err();
    assert!(refused.0.contains("checksum"), "{refused}");
}

#[test]
fn an_order_is_refused_rather_than_guessed_at() {
    let cases = [
        ("a short order", market("ETH", vec![Value::array([ALICE, "1", "2"])], Vec::new())),
        ("a user that is no address", market("ETH", vec![order("0xnope", "1", "2", 3)], Vec::new())),
        ("a price that is no decimal", market("ETH", Vec::new(), vec![order(ALICE, "1e3", "2", 3)])),
        ("a size that is no decimal", market("ETH", Vec::new(), vec![order(ALICE, "1", "two", 3)])),
        ("an oid that is a string", market("ETH", vec![Value::array([ALICE, "1", "2", "3"])], Vec::new())),
        ("a market with no asks", Value::array([Value::from("ETH"), Value::Array(Vec::new())])),
        ("a market that is a map", Value::Map(hl_core::Map::new().put("coin", "ETH"))),
    ];
    for (what, market) in cases {
        assert!(decode(&frame(1, &[blob(&market)])).is_err(), "{what}");
    }
}

#[test]
fn an_order_with_more_fields_than_four_keeps_the_four() {
    let longer = Value::array([
        Value::from(ALICE),
        Value::from("3245.5"),
        Value::from("1.5"),
        Value::Uint(217_148_811_876),
        Value::from("a field added later"),
    ]);
    let read = decode(&frame(1, &[blob(&market("ETH", vec![longer], Vec::new()))])).unwrap();
    assert_eq!(read.markets[0].bids[0].oid, 217_148_811_876);
    assert_eq!(read.markets[0].bids[0].px, dec("3245.5"));
}

#[test]
fn the_request_names_its_coins_or_asks_for_every_perp() {
    assert_eq!(l4_book(&["ETH", "#2870"]).body(), r##"{"type":"l4Book","coins":["ETH","#2870"]}"##);
    assert_eq!(l4_book(&[]).body(), r#"{"type":"l4Book"}"#);
    assert_eq!(l4_book(&["BTC"]).name(), "l4Book");
}
