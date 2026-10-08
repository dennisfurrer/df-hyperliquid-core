//! The worked examples of the RLP section of the Ethereum wiki and the
//! Yellow Paper's appendix B, as hip4-tui pins them, then viem's `toRlp`
//! over integers to 256 bits and payloads either side of 55 bytes.

use super::*;
use crate::{fixtures, u256::U256};

fn text(word: &str) -> Vec<u8> {
    bytes(word.as_bytes())
}

#[test]
fn strings_are_encoded_as_the_specification_shows() {
    assert_eq!(text("dog"), [0x83, b'd', b'o', b'g']);
    assert_eq!(text(""), [0x80]);
    assert_eq!(bytes(&[0x00]), [0x00], "a single low byte is itself");
    assert_eq!(bytes(&[0x0f]), [0x0f]);
    assert_eq!(bytes(&[0x04, 0x00]), [0x82, 0x04, 0x00]);
    assert_eq!(bytes(&[0x80]), [0x81, 0x80], "a single high byte is prefixed");
}

#[test]
fn a_string_of_fifty_six_bytes_takes_a_long_prefix() {
    let lorem = "Lorem ipsum dolor sit amet, consectetur adipisicing elit";
    assert_eq!(lorem.len(), 56);
    let encoded = text(lorem);
    assert_eq!(&encoded[..2], &[0xb8, 0x38]);
    assert_eq!(&encoded[2..], lorem.as_bytes());
    assert_eq!(&bytes(&[0xaa; 1024])[..3], &[0xb9, 0x04, 0x00]);
}

#[test]
fn integers_drop_their_leading_zeros() {
    assert_eq!(uint(0), [0x80], "zero is the empty string");
    assert_eq!(uint(15), [0x0f]);
    assert_eq!(uint(1024), [0x82, 0x04, 0x00]);
    assert_eq!(uint(127), [0x7f]);
    assert_eq!(uint(128), [0x81, 0x80]);
    let mut value = [0u8; 32];
    value[30] = 0x04;
    assert_eq!(word(&value), [0x82, 0x04, 0x00]);
    assert_eq!(word(&[0u8; 32]), [0x80]);
    assert_eq!(u256(U256::from(1024u64)), [0x82, 0x04, 0x00]);
    assert_eq!(u256(U256::ZERO), [0x80]);
}

#[test]
fn lists_nest_as_the_specification_shows() {
    assert_eq!(list(&[text("cat"), text("dog")]), [0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']);
    assert_eq!(list(&[]), [0xc0]);
    // The set theoretical representation of three: [ [], [[]], [ [], [[]] ] ].
    let empty = list(&[]);
    let one = list(std::slice::from_ref(&empty));
    let two = list(&[empty.clone(), one.clone()]);
    assert_eq!(hl_core::hex::encode(&list(&[empty, one, two])), "c7c0c1c0c3c0c1c0");
    let items: Vec<Vec<u8>> = (0..20).map(|_| text("dog")).collect();
    let encoded = list(&items);
    assert_eq!(&encoded[..2], &[0xf8, 80]);
    assert_eq!(encoded.len(), 82);
}

#[test]
fn every_case_matches_viem() {
    let viem = fixtures::viem();
    for case in viem["rlp"].as_array().unwrap() {
        let name = fixtures::text(&case["case"]);
        let encoded = match (case.get("list"), case.get("hashed")) {
            (Some(items), _) => {
                list(&items.as_array().unwrap().iter().map(|item| bytes(&fixtures::bytes(item))).collect::<Vec<_>>())
            }
            (None, Some(_)) => bytes(&[0xcc; 65_536]),
            (None, None) => {
                // viem's numberToHex leaves an odd digit count, which it reads as zero-padded.
                let digits = fixtures::text(&case["value"]).trim_start_matches("0x");
                let value = hl_core::hex::decode(&format!("{}{digits}", "0".repeat(digits.len() % 2))).unwrap();
                // viem writes integers trimmed; the crate's integer form must agree with it.
                if value.len() <= 32 && value.first() != Some(&0) {
                    assert_eq!(u256(U256::from_be_slice(&value).unwrap()), bytes(&value), "{name}");
                }
                bytes(&value)
            }
        };
        let expected = fixtures::text(&case["rlp"]);
        let got = match case.get("hashed") {
            Some(_) => hl_core::hex::encode_prefixed(&hl_core::keccak256(&encoded)),
            None => hl_core::hex::encode_prefixed(&encoded),
        };
        assert_eq!(got, expected, "{name}");
    }
}
