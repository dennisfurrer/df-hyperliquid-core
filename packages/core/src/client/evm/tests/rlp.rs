//! The worked examples of the RLP section of the Ethereum wiki and the Yellow Paper's appendix B,
//! then viem's `toRlp` over integers to 256 bits and payloads either side of 55 bytes.

use super::*;
use crate::client::evm::fixtures;
use crate::client::evm::u256::U256;
use crate::utils::hex;

fn text(word: &str) -> Vec<u8> {
    bytes(word.as_bytes())
}

#[test]
fn strings_and_integers_are_encoded_as_the_specification_shows() {
    let lorem = "Lorem ipsum dolor sit amet, consectetur adipisicing elit";
    let mut four_hundred = [0u8; 32];
    four_hundred[30] = 0x04;
    for (encoded, expected) in [
        (text("dog"), "83646f67"),
        (text(""), "80"),
        (bytes(&[0x00]), "00"),
        (bytes(&[0x0f]), "0f"),
        (bytes(&[0x04, 0x00]), "820400"),
        (bytes(&[0x80]), "8180"),
        (uint(0), "80"),
        (uint(15), "0f"),
        (uint(127), "7f"),
        (uint(128), "8180"),
        (uint(1024), "820400"),
        (word(&four_hundred), "820400"),
        (word(&[0u8; 32]), "80"),
        (u256(U256::from(1024u64)), "820400"),
        (u256(U256::ZERO), "80"),
    ] {
        assert_eq!(hex::encode(&encoded), expected);
    }
    // Fifty-six bytes take a long prefix: the tag, then the length.
    assert_eq!(lorem.len(), 56);
    assert_eq!(text(lorem), [&[0xb8, 0x38][..], lorem.as_bytes()].concat());
    assert_eq!(&bytes(&[0xaa; 1024])[..3], &[0xb9, 0x04, 0x00]);
}

#[test]
fn lists_nest_as_the_specification_shows() {
    assert_eq!(list(&[text("cat"), text("dog")]), [0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']);
    assert_eq!(list(&[]), [0xc0]);
    // The set theoretical representation of three: [ [], [[]], [ [], [[]] ] ].
    let empty = list(&[]);
    let one = list(std::slice::from_ref(&empty));
    let two = list(&[empty.clone(), one.clone()]);
    assert_eq!(hex::encode(&list(&[empty, one, two])), "c7c0c1c0c3c0c1c0");
    let items: Vec<Vec<u8>> = (0..20).map(|_| text("dog")).collect();
    let encoded = list(&items);
    assert_eq!((&encoded[..2], encoded.len()), (&[0xf8, 80][..], 82));
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
                let value = hex::decode(&format!("{}{digits}", "0".repeat(digits.len() % 2))).unwrap();
                // viem writes integers trimmed; the crate's integer form must agree with it.
                if value.len() <= 32 && value.first() != Some(&0) {
                    assert_eq!(u256(U256::from_be_slice(&value).unwrap()), bytes(&value), "{name}");
                }
                bytes(&value)
            }
        };
        let got = match case.get("hashed") {
            Some(_) => hex::encode_prefixed(&crate::utils::sign::keccak256(&encoded)),
            None => hex::encode_prefixed(&encoded),
        };
        assert_eq!(got, fixtures::text(&case["rlp"]), "{name}");
    }
}
