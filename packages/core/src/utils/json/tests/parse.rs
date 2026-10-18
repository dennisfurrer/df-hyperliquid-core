//! Text read as written, and a mistake placed where it is.

use super::*;

#[test]
fn keys_stay_in_the_order_they_were_typed() {
    let json = Json::parse(r#"{ "type": "cancel", "b": 1, "a": 2 }"#).unwrap();
    assert_eq!(json.compact(), r#"{"type":"cancel","b":1,"a":2}"#);
    let map = json.to_action().unwrap();
    assert_eq!(map.keys().collect::<Vec<_>>(), ["type", "b", "a"]);
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
