//! Hex both ways, and what a decode refuses.

use super::*;

#[test]
fn hex_is_lowercase_and_round_trips() {
    assert_eq!(encode(b"hello world"), "68656c6c6f20776f726c64");
    assert_eq!(encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    assert_eq!(encode_prefixed(&[0xde, 0xad, 0xbe, 0xef]), "0xdeadbeef");
    assert_eq!(encode(&[]), "");
    assert_eq!(encode_prefixed(&[]), "0x");
    assert_eq!(decode("0xDEADBEEF").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(decode("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(decode("  0xdeadbeef\n").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn a_fixed_size_decode_refuses_the_wrong_length() {
    let bytes: [u8; 4] = decode_array("0xdeadbeef").unwrap();
    assert_eq!(bytes, [0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(decode_array::<32>("0xdeadbeef"), Err(BadHex::WrongLength { expected: 32, found: 4 }));
    assert_eq!(decode("0xabc"), Err(BadHex::OddLength(3)));
    assert_eq!(decode("0xzz"), Err(BadHex::NotAHexDigit('z')));
}

#[test]
fn a_rejected_secret_is_never_quoted_back() {
    let error = decode_array::<32>("0xnot-a-key").unwrap_err();
    let text = error.to_string();
    assert!(!text.contains("not-a-key"), "{text} leaked the input");
}
