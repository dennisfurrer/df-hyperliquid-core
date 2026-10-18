//! Test helpers: a [`Token`] built from viem's JSON for a value, not by the code under test,
//! and bytes as `0x` hex.

use serde_json::Value;

use crate::client::evm::abi::{Kind, Token};
use crate::client::evm::fixtures;
use crate::client::evm::u256::U256;

/// A token from viem's JSON for a value: integers as decimal strings, hex for
/// addresses and bytes, arrays and tuples as lists (a named tuple as an object).
pub(crate) fn token(kind: &Kind, value: &Value) -> Token {
    match kind {
        Kind::Uint(_) => Token::Uint(fixtures::text(value).parse().unwrap()),
        Kind::Int(_) => {
            let text = fixtures::text(value);
            match text.strip_prefix('-') {
                // Two's complement of the magnitude: 2^256 - m.
                Some(magnitude) => {
                    let magnitude: U256 = magnitude.parse().unwrap();
                    Token::Int(U256::MAX.checked_sub(magnitude.checked_sub(U256::ONE).unwrap()).unwrap())
                }
                None => Token::Int(text.parse().unwrap()),
            }
        }
        Kind::Address => Token::Address(fixtures::address(fixtures::text(value))),
        Kind::Bool => Token::Bool(value.as_bool().unwrap()),
        Kind::String => Token::String(fixtures::text(value).to_owned()),
        Kind::Bytes => Token::Bytes(fixtures::bytes(value)),
        Kind::Fixed(_) => Token::Fixed(fixtures::bytes(value)),
        Kind::Array(inner) | Kind::Sized(inner, _) => {
            Token::Array(value.as_array().unwrap().iter().map(|item| token(inner, item)).collect())
        }
        Kind::Tuple(params) => Token::Tuple(
            params
                .iter()
                .enumerate()
                .map(|(at, param)| match (value, &param.name) {
                    (Value::Object(fields), Some(name)) => token(&param.kind, &fields[name]),
                    _ => token(&param.kind, &value[at]),
                })
                .collect(),
        ),
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    crate::utils::hex::encode_prefixed(bytes)
}
