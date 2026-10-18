//! Output as `JSON.stringify` writes it, and indented output that reads back the same.

use super::*;

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
