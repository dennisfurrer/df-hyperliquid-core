//! The msgpack a JSON value hashes as, and what has no msgpack the venue would hash the same way.

use super::*;
use crate::utils::hex;

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

/// A fraction or an exponent would be a float, and a float is never what an action means.
#[test]
fn a_fraction_a_number_past_64_bits_or_anything_but_an_object_is_refused() {
    for (text, refused) in [
        ("1.5", Unpackable::Fraction("1.5".to_owned())),
        ("[2e3]", Unpackable::Fraction("2e3".to_owned())),
        ("18446744073709551616", Unpackable::TooWide("18446744073709551616".to_owned())),
        ("-9223372036854775809", Unpackable::TooWide("-9223372036854775809".to_owned())),
    ] {
        assert_eq!(Json::parse(text).unwrap().to_msgpack(), Err(refused), "{text}");
    }
    assert_eq!(Json::parse("[1]").unwrap().to_action(), Err(Unpackable::NotAnObject("an array")));
    assert_eq!(Json::parse(r#"{"n":1.5}"#).unwrap().to_action(), Err(Unpackable::Fraction("1.5".to_owned())));
}
