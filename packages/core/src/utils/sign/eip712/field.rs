//! One field of a typed struct: its Solidity type, its word in the struct hash, and how a wallet and
//! the posted action each carry its value.

use crate::utils::address::Address;
use crate::utils::hex;
use crate::utils::json::Json;
use crate::utils::sign::eip712::word::{address_word, uint256, word, word_decimal};
use crate::utils::sign::keccak::{Keccak256, keccak256};
use crate::utils::sign::msgpack::Value;

/// A typed struct field's value, tagged with its Solidity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field<'a> {
    /// `string`, hashed as the keccak of its UTF-8 bytes.
    String(&'a str),
    /// `address`.
    Address(Address),
    /// `uint64`.
    Uint64(u64),
    /// `bytes32`, carried as `0x` and lowercase hex.
    Bytes32([u8; 32]),
    /// `bool`.
    Bool(bool),
    /// `uint32`.
    Uint32(u32),
    /// `uint128`, carried as a decimal string.
    Uint128(u128),
    /// The big-endian word; [`uint256`] makes one from a smaller number.
    Uint256([u8; 32]),
    /// Each element's big-endian word, in order.
    Uint256Array(&'a [[u8; 32]]),
    /// Bytes of any length, carried as `0x` and lowercase hex.
    Bytes(&'a [u8]),
}

impl Field<'_> {
    /// The type's name in the struct's type string, such as `uint64` or `uint256[]`.
    #[must_use]
    pub const fn solidity_type(&self) -> &'static str {
        match self {
            Field::String(_) => "string",
            Field::Address(_) => "address",
            Field::Uint64(_) => "uint64",
            Field::Bytes32(_) => "bytes32",
            Field::Bool(_) => "bool",
            Field::Uint32(_) => "uint32",
            Field::Uint128(_) => "uint128",
            Field::Uint256(_) => "uint256",
            Field::Uint256Array(_) => "uint256[]",
            Field::Bytes(_) => "bytes",
        }
    }

    /// The field's word in the struct hash. An array's is the keccak of its
    /// elements' words, as EIP-712 encodes one.
    #[must_use]
    pub fn encode(&self) -> [u8; 32] {
        match self {
            Field::String(text) => keccak256(text.as_bytes()),
            Field::Address(address) => address_word(*address),
            Field::Uint64(number) => word(*number),
            Field::Bytes32(bytes) | Field::Uint256(bytes) => *bytes,
            Field::Bool(value) => word(u64::from(*value)),
            Field::Uint32(number) => word(u64::from(*number)),
            Field::Uint128(number) => uint256(*number),
            Field::Uint256Array(words) => {
                words.iter().fold(Keccak256::new(), |sponge, element| sponge.chain(element)).finalize()
            }
            Field::Bytes(bytes) => keccak256(bytes),
        }
    }

    /// The value as a wallet's `message` carries it: numbers as JSON numbers,
    /// as viem passes them, bools as JSON bools, the rest as strings. A
    /// `uint128` or `uint256` is a decimal string, as viem sends a bigint:
    /// JavaScript reads a JSON number past 2^53 inexactly.
    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Field::String(text) => Json::String((*text).to_owned()),
            Field::Address(address) => Json::String(address.to_hex()),
            Field::Uint64(number) => Json::Number(number.to_string()),
            Field::Bytes32(bytes) => Json::String(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Json::Bool(*value),
            Field::Uint32(number) => Json::Number(number.to_string()),
            Field::Uint128(number) => Json::String(number.to_string()),
            Field::Uint256(bytes) => Json::String(word_decimal(bytes)),
            Field::Uint256Array(words) => {
                Json::Array(words.iter().map(|element| Json::String(word_decimal(element))).collect())
            }
            Field::Bytes(bytes) => Json::String(hex::encode_prefixed(bytes)),
        }
    }

    /// The value as the posted action carries it: addresses lowercase. A
    /// number past 64 bits is its decimal string, as no msgpack integer holds it.
    #[must_use]
    pub fn to_wire(&self) -> Value {
        match self {
            Field::String(text) => Value::from((*text).to_owned()),
            Field::Address(address) => Value::from(address.to_hex()),
            Field::Uint64(number) => Value::Uint(*number),
            Field::Bytes32(bytes) => Value::from(hex::encode_prefixed(bytes)),
            Field::Bool(value) => Value::Bool(*value),
            Field::Uint32(number) => Value::Uint(u64::from(*number)),
            Field::Uint128(number) => Value::from(number.to_string()),
            Field::Uint256(bytes) => Value::from(word_decimal(bytes)),
            Field::Uint256Array(words) => {
                Value::Array(words.iter().map(|element| Value::from(word_decimal(element))).collect())
            }
            Field::Bytes(bytes) => Value::from(hex::encode_prefixed(bytes)),
        }
    }
}

#[cfg(test)]
#[path = "tests/field.rs"]
mod tests;
