use super::*;
use crate::hex;

#[test]
fn keys_stay_in_the_order_they_were_typed() {
    let json = Json::parse(r#"{ "type": "cancel", "b": 1, "a": 2 }"#).unwrap();
    assert_eq!(json.compact(), r#"{"type":"cancel","b":1,"a":2}"#);
    let map = json.to_action().unwrap();
    assert_eq!(map.keys().collect::<Vec<_>>(), ["type", "b", "a"]);
}

#[test]
fn numbers_keep_the_digits_they_were_written_with() {
    let json = Json::parse("[123456789012345678901234567890, -1.50e3, 0]").unwrap();
    assert_eq!(json.compact(), "[123456789012345678901234567890,-1.50e3,0]");
    assert_eq!(Json::parse("1.5").unwrap().to_msgpack(), Err(Unpackable::Fraction("1.5".to_owned())));
    assert!(matches!(Json::parse("18446744073709551616").unwrap().to_msgpack(), Err(Unpackable::TooWide(_))));
}

#[test]
fn msgpack_matches_what_the_reference_encoder_writes() {
    // `@msgpack/msgpack` over the same object, captured from node.
    let json = Json::parse(
        r#"{"type":"cancel","cancels":[{"a":100000000,"o":5000000000},{"a":-1,"o":0}],"flag":true,"none":null}"#,
    )
    .unwrap();
    assert_eq!(
        hex::encode(&json.to_msgpack().unwrap().to_msgpack()),
        "84a474797065a663616e63656ca763616e63656c739282a161ce05f5e100a16fcf000000012a05f20082a161ffa16f00a4666c6167c3a46e6f6e65c0"
    );
}

#[test]
fn pretty_is_two_spaces_a_level_and_reads_back_the_same() {
    let text = "{\n  \"type\": \"outcomeMeta\",\n  \"list\": [\n    1,\n    {}\n  ],\n  \"none\": []\n}";
    let json = Json::parse(text).unwrap();
    assert_eq!(json.pretty(), text);
    assert_eq!(Json::parse(&json.pretty()).unwrap(), json);
}

#[test]
fn strings_escape_as_json_stringify_does() {
    let json = Json::parse(r#""a\"b\\c\n\u0001é😀/""#).unwrap();
    assert_eq!(json, Json::String("a\"b\\c\n\u{1}\u{e9}\u{1f600}/".to_owned()));
    assert_eq!(json.compact(), "\"a\\\"b\\\\c\\n\\u0001\u{e9}\u{1f600}/\"");
}

#[test]
fn a_mistake_is_placed_by_line_and_column() {
    let wrong = Json::parse("{\n  \"type\": outcomeMeta\n}").unwrap_err();
    assert_eq!((wrong.line, wrong.column), (2, 11));
    assert!(Json::parse("{\"a\":1,\"a\":2}").unwrap_err().message.contains("twice"));
    assert!(Json::parse("[1,]").is_err());
    assert!(Json::parse("01").is_err());
    assert!(Json::parse("{} x").is_err());
    assert!(Json::parse("\"open").is_err());
}

#[test]
fn only_an_object_is_an_action() {
    assert_eq!(Json::parse("[1]").unwrap().to_action(), Err(Unpackable::NotAnObject("an array")));
}
