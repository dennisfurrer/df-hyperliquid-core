//! Typed data read from its JSON alone: viem's digests for every Papertrade intent, the
//! specification's own example, and what does not read refused.

use super::*;
use crate::utils::json::Json;

const VIEM: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/papertrade_typed_viem.json"));

fn hex32(text: &str) -> [u8; 32] {
    hex::decode_array(text).unwrap()
}

/// Every intent a Papertrade wallet or session key signs, hashed by viem: the
/// digest of the wallet JSON alone must be viem's, field types and all
/// (uint32 to uint256, a uint256 list past 128 bits, bytes32, bool, address).
#[test]
fn every_case_hashes_as_viem_hashes_it() {
    let fixture = Json::parse(VIEM).unwrap();
    let cases = fixture.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 13);
    for case in cases {
        let name = case.get("name").and_then(Json::as_str).unwrap();
        let wallet = case.get("walletJson").and_then(Json::as_str).unwrap();
        let want = hex32(case.get("digest").and_then(Json::as_str).unwrap());
        let read = read(wallet).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(read.digest, want, "{name}");
        assert_eq!(read.primary, case.get("primaryType").and_then(Json::as_str).unwrap(), "{name}");
    }
}

/// The specification's own example: a struct inside a struct.
#[test]
fn the_specifications_mail_hashes_as_published() {
    let mail = r#"{
      "types": {
        "EIP712Domain": [
          {"name": "name", "type": "string"},
          {"name": "version", "type": "string"},
          {"name": "chainId", "type": "uint256"},
          {"name": "verifyingContract", "type": "address"}
        ],
        "Person": [
          {"name": "name", "type": "string"},
          {"name": "wallet", "type": "address"}
        ],
        "Mail": [
          {"name": "from", "type": "Person"},
          {"name": "to", "type": "Person"},
          {"name": "contents", "type": "string"}
        ]
      },
      "primaryType": "Mail",
      "domain": {
        "name": "Ether Mail",
        "version": "1",
        "chainId": 1,
        "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
      },
      "message": {
        "from": {"name": "Cow", "wallet": "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826"},
        "to": {"name": "Bob", "wallet": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"},
        "contents": "Hello, Bob!"
      }
    }"#;
    let read = read(mail).unwrap();
    assert_eq!(read.digest, hex32("0xbe609aee343fb3c4b28e1df9e632fca64fcfaede20f02e86244efddf30957bd2"));
    assert_eq!(read.message[2], ("contents".to_owned(), "Hello, Bob!".to_owned()));
    assert_eq!(read.domain[0], ("name".to_owned(), "Ether Mail".to_owned()));
}

fn one_field(kind: &str, value: &str) -> String {
    format!(
        r#"{{"types":{{"EIP712Domain":[{{"name":"name","type":"string"}}],"T":[{{"name":"v","type":"{kind}"}}]}},"primaryType":"T","domain":{{"name":"x"}},"message":{{"v":{value}}}}}"#
    )
}

#[test]
fn numbers_are_whole_and_in_range() {
    assert!(digest(&one_field("uint8", "255")).is_ok());
    assert!(digest(&one_field("uint8", "256")).is_err());
    assert!(digest(&one_field("uint256", "\"0xff\"")).is_ok());
    assert!(digest(&one_field("uint64", "1.5")).is_err());
    assert!(digest(&one_field("uint64", "-1")).is_err());
    assert!(digest(&one_field("int8", "-128")).is_ok());
    assert!(digest(&one_field("int8", "-129")).is_err());
    assert!(digest(&one_field("int8", "127")).is_ok());
    assert!(digest(&one_field("int8", "128")).is_err());
    assert!(digest(&one_field("uint7", "1")).is_err(), "widths go by eights");
    let over = format!("\"1{}\"", "0".repeat(78));
    assert!(digest(&one_field("uint256", &over)).is_err(), "past 2^256");
}

#[test]
fn a_negative_is_its_twos_complement() {
    let minus_one = digest(&one_field("int256", "-1")).unwrap();
    let all_ones = digest(&one_field("uint256", &format!("\"0x{}\"", "f".repeat(64)))).unwrap();
    assert_ne!(minus_one, all_ones, "the type is part of the hash");
    assert_eq!(signed(true, from_decimal("1").unwrap(), 256), Some([0xff; 32]));
    let mut minus_256 = [0xff; 32];
    minus_256[31] = 0;
    assert_eq!(signed(true, from_decimal("256").unwrap(), 256), Some(minus_256));
}

#[test]
fn what_does_not_read_is_refused() {
    assert!(digest("not json").is_err());
    assert!(digest(&one_field("uint512", "1")).is_err());
    assert!(digest(&one_field("address", "\"0x12\"")).is_err());
    assert!(digest(&one_field("bytes4", "\"0x1234\"")).is_err());
    assert!(digest(&one_field("bytes2", "\"0x1234\"")).is_ok());
    assert!(digest(&one_field("uint256[2]", "[1]")).is_err());
    assert!(digest(&one_field("uint256[]", "[1,2,3]")).is_ok());
    let missing =
        r#"{"types":{"EIP712Domain":[],"T":[{"name":"v","type":"bool"}]},"primaryType":"T","domain":{},"message":{}}"#;
    assert!(matches!(digest(missing), Err(BadTypedData::Shape(why)) if why.contains("no `v`")));
    let selfish = r#"{"types":{"EIP712Domain":[],"T":[{"name":"t","type":"T"}]},"primaryType":"T","domain":{},"message":{"t":{"t":{"t":{}}}}}"#;
    assert!(digest(selfish).is_err(), "a type that names itself never ends");
}
