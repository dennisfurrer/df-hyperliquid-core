//! Msgpack as the venue hashes an L1 action: the smallest header for each value, a non-negative
//! integer always unsigned, map keys in insertion order, and an absent key left out.

use super::*;
use crate::utils::hex;

#[test]
fn each_scalar_takes_the_smallest_header_and_a_non_negative_integer_goes_unsigned() {
    for (value, bytes) in [
        (Value::Nil, &[0xc0][..]),
        (Value::Bool(false), &[0xc2]),
        (Value::Bool(true), &[0xc3]),
        (Value::Uint(0), &[0x00]),
        (Value::Uint(127), &[0x7f]),
        (Value::Uint(128), &[0xcc, 0x80]),
        (Value::Uint(255), &[0xcc, 0xff]),
        (Value::Uint(256), &[0xcd, 0x01, 0x00]),
        (Value::Uint(65_535), &[0xcd, 0xff, 0xff]),
        (Value::Uint(65_536), &[0xce, 0x00, 0x01, 0x00, 0x00]),
        (Value::Uint(100_005_160), &[0xce, 0x05, 0xf5, 0xf5, 0x28]),
        (Value::Uint(4_294_967_296), &[0xcf, 0, 0, 0, 1, 0, 0, 0, 0]),
        (Value::Uint(1_712_345_678_000), &[0xcf, 0x00, 0x00, 0x01, 0x8e, 0xaf, 0xc1, 0x80, 0xb0]),
        (Value::Int(100_005_160), &[0xce, 0x05, 0xf5, 0xf5, 0x28]),
        (Value::Int(0), &[0x00]),
        (Value::Int(-1), &[0xff]),
        (Value::Int(-32), &[0xe0]),
        (Value::Int(-33), &[0xd0, 0xdf]),
        (Value::Int(-129), &[0xd1, 0xff, 0x7f]),
        (Value::Int(-32_769), &[0xd2, 0xff, 0xff, 0x7f, 0xff]),
        (Value::Int(-2_147_483_649), &[0xd3, 0xff, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff]),
    ] {
        assert_eq!(encode(&value), bytes, "{value:?}");
    }
}

#[test]
fn strings_and_containers_pick_the_right_header() {
    let text = |len: usize| Value::from("a".repeat(len));
    let array = |len: u64| Value::array((0..len).map(|_| Value::Nil));
    let map = |len: u64| Value::Map((0..len).fold(Map::new(), |map, at| map.put(at.to_string(), at)));
    for (value, header) in [
        (Value::from("abc"), &[0xa3, b'a'][..]),
        (Value::from(""), &[0xa0]),
        (text(31), &[0xbf]),
        (text(32), &[0xd9, 32]),
        (text(300), &[0xda, 0x01, 0x2c]),
        (text(70_000), &[0xdb, 0x00, 0x01, 0x11, 0x70]),
        (Value::array([1u64, 2, 3]), &[0x93, 0x01]),
        (array(16), &[0xdc, 0x00, 0x10]),
        (array(70_000), &[0xdd, 0x00, 0x01, 0x11, 0x70]),
        (map(15), &[0x8f]),
        (map(16), &[0xde, 0x00, 0x10]),
        (map(70_000), &[0xdf, 0x00, 0x01, 0x11, 0x70]),
    ] {
        assert_eq!(&encode(&value)[..header.len()], header);
    }
}

fn canonical_order_action() -> Map {
    let limit = Map::new().put("tif", "FrontendMarket");
    let order = Map::new()
        .put("a", 100_005_160u64)
        .put("b", true)
        .put("p", "0.35")
        .put("s", "50")
        .put("r", false)
        .put("t", Map::new().put("limit", limit));
    Map::new().put("type", "order").put("orders", Value::array([order])).put("grouping", "na")
}

#[test]
fn the_canonical_order_action_encodes_byte_for_byte_in_the_order_written() {
    let bytes = canonical_order_action().to_msgpack();
    assert_eq!(bytes.len(), 87, "encoded {}", hex::encode(&bytes));
    #[rustfmt::skip]
    let expected: Vec<u8> = [
        &[0x83][..],                                  // fixmap, 3 keys
        &[0xa4], b"type",      &[0xa5], b"order",
        &[0xa6], b"orders",
        &[0x91],                                      // fixarray, 1 order
        &[0x86],                                      // fixmap, 6 keys
        &[0xa1], b"a",         &[0xce, 0x05, 0xf5, 0xf5, 0x28],
        &[0xa1], b"b",         &[0xc3],
        &[0xa1], b"p",         &[0xa4], b"0.35",
        &[0xa1], b"s",         &[0xa2], b"50",
        &[0xa1], b"r",         &[0xc2],
        &[0xa1], b"t",         &[0x81],
        &[0xa5], b"limit",     &[0x81],
        &[0xa3], b"tif",       &[0xae], b"FrontendMarket",
        &[0xa8], b"grouping",  &[0xa2], b"na",
    ]
    .concat();
    assert_eq!(bytes, expected, "got {}", hex::encode(&bytes));
    assert_eq!(Value::Map(canonical_order_action()).to_msgpack(), expected);
    assert_eq!(canonical_order_action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping"]);
    assert_eq!(
        canonical_order_action().to_json().compact(),
        r#"{"type":"order","orders":[{"a":100005160,"b":true,"p":"0.35","s":"50","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#,
        "what is posted is what was hashed"
    );

    let alphabetical = Map::new()
        .put("grouping", "na")
        .put("orders", Value::array([Map::new().put("a", 1u64)]))
        .put("type", "order")
        .to_msgpack();
    assert_ne!(bytes, alphabetical);
    assert_ne!(
        Map::new().put("b", 1u64).put("a", 2u64).to_msgpack(),
        Map::new().put("a", 2u64).put("b", 1u64).to_msgpack()
    );
}

#[test]
fn an_absent_optional_key_is_omitted_rather_than_nulled() {
    let with = Map::new().put("a", 1u64).put_some("c", Some("0x01"));
    let without = Map::new().put("a", 1u64).put_some("c", None::<&str>);
    assert_eq!((with.len(), without.len()), (2, 1));
    assert_eq!(with.get("c"), Some(&Value::from("0x01")));
    let nulled = Map::new().put("a", 1u64).put("c", Value::Nil);
    assert_ne!(without.to_msgpack(), nulled.to_msgpack());
    assert_eq!((without.to_msgpack()[0], nulled.to_msgpack()[0]), (0x81, 0x82));
}
