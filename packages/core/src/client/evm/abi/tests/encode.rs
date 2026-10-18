//! Encodings against viem's `encodeAbiParameters` over the same types and values, each
//! case round-tripped by viem first, and recorded cases from typed text. The values are built
//! from the JSON, not by the code under test.

use crate::client::evm::abi::support::{hex, token};
use crate::client::evm::abi::{decode, params};
use crate::client::evm::fixtures;

use super::*;

#[test]
fn every_viem_and_recorded_case_encodes_byte_for_byte() {
    let viem = fixtures::viem();
    let cases = viem["abi"].as_array().unwrap();
    assert_eq!(cases.len(), 76, "every width of uint and int, and the rest");
    for case in cases {
        let types = fixtures::text(&case["types"]);
        let shape = params(types).unwrap();
        let tokens: Vec<Token> = shape
            .iter()
            .zip(case["values"].as_array().unwrap())
            .map(|(param, value)| token(&param.kind, value))
            .collect();
        let encoded = fixtures::bytes(&case["encoded"]);
        assert_eq!(hex(&encode(&shape, &tokens).unwrap()), fixtures::text(&case["encoded"]), "{types}");
        assert_eq!(decode(&shape, &encoded).unwrap(), tokens, "{types}");
    }
    // The same from typed text, as a person writes the values.
    let file = fixtures::cases();
    for case in file["abi"]["encoded"].as_array().unwrap() {
        let types = fixtures::text(&case["types"]);
        let shape = params(types).unwrap();
        let tokens: Vec<Token> = shape
            .iter()
            .zip(case["values"].as_array().unwrap())
            .map(|(param, value)| Token::parse(&param.kind, "value", fixtures::text(value)).unwrap())
            .collect();
        assert_eq!(hex(&encode(&shape, &tokens).unwrap()), fixtures::text(&case["encoded"]), "{types}");
    }
}

#[test]
fn a_value_that_does_not_fit_its_type_is_refused_rather_than_encoded() {
    let uint8 = params("uint8").unwrap();
    assert!(matches!(encode(&uint8, &[Token::uint(256u64)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&uint8, &[Token::Bool(true)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&uint8, &[]), Err(AbiError::Count { wants: 1, got: 0 })));
    let int8 = params("int8").unwrap();
    assert!(encode(&int8, &[Token::int(-128)]).is_ok());
    assert!(matches!(encode(&int8, &[Token::int(128)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&int8, &[Token::int(-129)]), Err(AbiError::Mismatch { .. })));
    let fixed = params("bytes4").unwrap();
    assert!(matches!(encode(&fixed, &[Token::Fixed(vec![1, 2, 3])]), Err(AbiError::Mismatch { .. })));
    let sized = params("uint8[2]").unwrap();
    assert!(matches!(encode(&sized, &[Token::Array(vec![Token::uint(1u8)])]), Err(AbiError::Mismatch { .. })));
}
