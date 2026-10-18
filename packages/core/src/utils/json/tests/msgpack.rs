//! The msgpack a JSON value hashes as.

use super::*;
use crate::utils::hex;

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
fn only_an_object_is_an_action() {
    assert_eq!(Json::parse("[1]").unwrap().to_action(), Err(Unpackable::NotAnObject("an array")));
}
