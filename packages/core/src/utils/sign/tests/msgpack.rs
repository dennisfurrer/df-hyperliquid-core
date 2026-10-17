use super::*;
use crate::utils::hex;

#[test]
fn the_scalar_encodings_match_the_spec() {
    assert_eq!(encode(&Value::Nil), vec![0xc0]);
    assert_eq!(encode(&Value::Bool(false)), vec![0xc2]);
    assert_eq!(encode(&Value::Bool(true)), vec![0xc3]);
    assert_eq!(encode(&Value::Uint(0)), vec![0x00]);
    assert_eq!(encode(&Value::Uint(127)), vec![0x7f]);
    assert_eq!(encode(&Value::Uint(128)), vec![0xcc, 0x80]);
    assert_eq!(encode(&Value::Uint(255)), vec![0xcc, 0xff]);
    assert_eq!(encode(&Value::Uint(256)), vec![0xcd, 0x01, 0x00]);
    assert_eq!(encode(&Value::Uint(65_535)), vec![0xcd, 0xff, 0xff]);
    assert_eq!(encode(&Value::Uint(65_536)), vec![0xce, 0x00, 0x01, 0x00, 0x00]);
    assert_eq!(encode(&Value::Uint(100_005_160)), vec![0xce, 0x05, 0xf5, 0xf5, 0x28]);
    assert_eq!(encode(&Value::Uint(4_294_967_296)), vec![0xcf, 0, 0, 0, 1, 0, 0, 0, 0]);
    assert_eq!(encode(&Value::Uint(1_712_345_678_000)), vec![0xcf, 0x00, 0x00, 0x01, 0x8e, 0xaf, 0xc1, 0x80, 0xb0]);
}

#[test]
fn a_non_negative_signed_value_still_goes_out_unsigned() {
    assert_eq!(encode(&Value::Int(100_005_160)), vec![0xce, 0x05, 0xf5, 0xf5, 0x28]);
    assert_eq!(encode(&Value::Int(0)), vec![0x00]);
    assert_eq!(encode(&Value::Int(-1)), vec![0xff]);
    assert_eq!(encode(&Value::Int(-32)), vec![0xe0]);
    assert_eq!(encode(&Value::Int(-33)), vec![0xd0, 0xdf]);
    assert_eq!(encode(&Value::Int(-129)), vec![0xd1, 0xff, 0x7f]);
    assert_eq!(encode(&Value::Int(-32_769)), vec![0xd2, 0xff, 0xff, 0x7f, 0xff]);
    assert_eq!(encode(&Value::Int(-2_147_483_649)), vec![0xd3, 0xff, 0xff, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff]);
}

#[test]
fn strings_and_containers_pick_the_right_header() {
    assert_eq!(encode(&Value::from("abc")), vec![0xa3, b'a', b'b', b'c']);
    assert_eq!(encode(&Value::from("")), vec![0xa0]);
    let thirty_one = "a".repeat(31);
    assert_eq!(encode(&Value::from(thirty_one))[0], 0xbf);
    let thirty_two = "a".repeat(32);
    assert_eq!(&encode(&Value::from(thirty_two))[..2], &[0xd9, 32]);
    let long = "a".repeat(300);
    assert_eq!(&encode(&Value::from(long))[..3], &[0xda, 0x01, 0x2c]);

    assert_eq!(encode(&Value::array([1u64, 2, 3]))[0], 0x93);
    let wide = Value::array((0u64..16).map(Value::Uint));
    assert_eq!(&encode(&wide)[..3], &[0xdc, 0x00, 0x10]);

    let mut map = Map::new();
    for index in 0..16u64 {
        map = map.put(index.to_string(), index);
    }
    assert_eq!(&encode(&Value::Map(map))[..3], &[0xde, 0x00, 0x10]);
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
fn the_canonical_order_action_encodes_byte_for_byte() {
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
}

#[test]
fn a_map_never_sorts_itself() {
    let wire = canonical_order_action().to_msgpack();
    let alphabetical = Map::new()
        .put("grouping", "na")
        .put("orders", Value::array([Map::new().put("a", 1u64)]))
        .put("type", "order")
        .to_msgpack();
    assert_ne!(wire, alphabetical);
    assert_eq!(canonical_order_action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping"]);
    let swapped = Map::new().put("b", 1u64).put("a", 2u64).to_msgpack();
    let ordered = Map::new().put("a", 2u64).put("b", 1u64).to_msgpack();
    assert_ne!(swapped, ordered);
}

#[test]
fn an_absent_optional_key_is_omitted_rather_than_nulled() {
    let with = Map::new().put("a", 1u64).put_some("c", Some("0x01"));
    let without = Map::new().put("a", 1u64).put_some("c", None::<&str>);
    assert_eq!(without.len(), 1);
    assert_eq!(with.len(), 2);
    let nulled = Map::new().put("a", 1u64).put("c", Value::Nil);
    assert_ne!(without.to_msgpack(), nulled.to_msgpack());
    assert_eq!(without.to_msgpack()[0], 0x81);
    assert_eq!(nulled.to_msgpack()[0], 0x82);
}
