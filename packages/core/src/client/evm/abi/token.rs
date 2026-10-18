//! A [`Token`], a value of some [`Kind`](crate::client::evm::abi::Kind): built
//! from Rust values and read back into them, signed integers in two's complement.

use crate::client::evm::u256::U256;
use crate::utils::Address;

/// A value of some [`Kind`](crate::client::evm::abi::Kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Uint(U256),
    /// Two's complement in 256 bits: -1 is all ones, whatever the width.
    Int(U256),
    Address(Address),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    /// The `n` bytes of a `bytesN`.
    Fixed(Vec<u8>),
    /// `T[]` and `T[n]`.
    Array(Vec<Token>),
    Tuple(Vec<Token>),
}

impl Token {
    #[must_use]
    pub fn uint(value: impl Into<U256>) -> Token {
        Token::Uint(value.into())
    }

    #[must_use]
    pub fn int(value: i128) -> Token {
        let magnitude = U256::from_u128(value.unsigned_abs());
        Token::Int(match value < 0 {
            true => negated(magnitude),
            false => magnitude,
        })
    }

    #[must_use]
    pub fn address(address: Address) -> Token {
        Token::Address(address)
    }

    #[must_use]
    pub fn as_u256(&self) -> Option<U256> {
        match self {
            Token::Uint(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_u128(&self) -> Option<u128> {
        self.as_u256().and_then(U256::to_u128)
    }

    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.as_u256().and_then(U256::to_u64)
    }

    #[must_use]
    pub fn as_u32(&self) -> Option<u32> {
        self.as_u64().and_then(|value| u32::try_from(value).ok())
    }

    #[must_use]
    pub fn as_u16(&self) -> Option<u16> {
        self.as_u64().and_then(|value| u16::try_from(value).ok())
    }

    #[must_use]
    pub fn as_u8(&self) -> Option<u8> {
        self.as_u64().and_then(|value| u8::try_from(value).ok())
    }

    /// A signed integer, while it fits an `i128`.
    #[must_use]
    pub fn as_i128(&self) -> Option<i128> {
        let Token::Int(value) = self else { return None };
        match value.bits() == 256 {
            false => value.to_u128().and_then(|value| i128::try_from(value).ok()),
            true => {
                let magnitude = negated(*value).to_u128()?;
                match magnitude == i128::MIN.unsigned_abs() {
                    true => Some(i128::MIN),
                    false => i128::try_from(magnitude).ok().map(|magnitude| -magnitude),
                }
            }
        }
    }

    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.as_i128().and_then(|value| i64::try_from(value).ok())
    }

    #[must_use]
    pub fn as_i8(&self) -> Option<i8> {
        self.as_i128().and_then(|value| i8::try_from(value).ok())
    }

    #[must_use]
    pub fn as_address(&self) -> Option<Address> {
        match self {
            Token::Address(address) => Some(*address),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Token::Bool(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Token::String(text) => Some(text),
            _ => None,
        }
    }

    /// The bytes of a `bytes` or a `bytesN`.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Token::Bytes(bytes) | Token::Fixed(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The items of an array or the fields of a tuple.
    #[must_use]
    pub fn as_items(&self) -> Option<&[Token]> {
        match self {
            Token::Array(items) | Token::Tuple(items) => Some(items),
            _ => None,
        }
    }
}

/// `-value` in two's complement: `2^256 - value`, and zero for zero.
pub(super) fn negated(value: U256) -> U256 {
    match value.checked_sub(U256::ONE) {
        Some(less) => U256::MAX.checked_sub(less).unwrap_or(U256::ZERO),
        None => U256::ZERO,
    }
}
