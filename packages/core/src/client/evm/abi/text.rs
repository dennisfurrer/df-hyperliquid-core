//! Tokens as text: a typed value read into its kind and range-checked, and a
//! value shown back the way a person reads it.

use crate::client::evm::abi::kind::split;
use crate::client::evm::abi::token::negated;
use crate::client::evm::abi::{AbiError, Kind, Token, WORD};
use crate::client::evm::u256::U256;
use crate::utils::hex;

impl Token {
    /// A typed value read into this kind: decimal for integers, `0x` hex for
    /// addresses and bytes, `true` or `false`, and `[a, b]` or `(a, b)` for
    /// the containers. `name` is what a refusal calls the field.
    pub fn parse(kind: &Kind, name: &str, text: &str) -> Result<Token, AbiError> {
        let text = text.trim();
        let refused = || AbiError::Value { name: name.to_owned(), wants: kind.wants(), got: text.to_owned() };
        Ok(match kind {
            Kind::Uint(bits) => {
                let value = U256::from_decimal(text).map_err(|_| refused())?;
                if value.bits() > u32::from(*bits) {
                    return Err(refused());
                }
                Token::Uint(value)
            }
            Kind::Int(bits) => {
                let (negative, digits) = match text.strip_prefix('-') {
                    Some(rest) => (true, rest),
                    None => (false, text),
                };
                let magnitude = U256::from_decimal(digits).map_err(|_| refused())?;
                // One bit is the sign: a magnitude below 2^(bits-1), or exactly
                // that when it is negative.
                let room = u32::from(*bits) - 1;
                let edge = magnitude.bits() == room + 1 && magnitude == power_of_two(room);
                if magnitude.bits() > room && !(negative && edge) {
                    return Err(refused());
                }
                Token::Int(match negative {
                    true => negated(magnitude),
                    false => magnitude,
                })
            }
            Kind::Address => Token::Address(text.parse().map_err(|_| refused())?),
            Kind::Bool => match text.to_ascii_lowercase().as_str() {
                "true" => Token::Bool(true),
                "false" => Token::Bool(false),
                _ => return Err(refused()),
            },
            Kind::String => Token::String(text.to_owned()),
            Kind::Bytes => Token::Bytes(hex_of(text).ok_or_else(refused)?),
            Kind::Fixed(size) => Token::Fixed(hex_of(text).filter(|bytes| bytes.len() == *size).ok_or_else(refused)?),
            Kind::Array(inner) | Kind::Sized(inner, _) => {
                let inside = text.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).ok_or_else(refused)?;
                let items: Vec<&str> = match inside.trim().is_empty() {
                    true => Vec::new(),
                    false => split(inside),
                };
                if let Kind::Sized(_, count) = kind
                    && items.len() != *count
                {
                    return Err(refused());
                }
                Token::Array(items.iter().map(|item| Token::parse(inner, name, item)).collect::<Result<_, _>>()?)
            }
            Kind::Tuple(params) => {
                let inside = text.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')).ok_or_else(refused)?;
                let items = split(inside);
                if items.len() != params.len() {
                    return Err(refused());
                }
                Token::Tuple(
                    params
                        .iter()
                        .zip(items)
                        .map(|(param, item)| Token::parse(&param.kind, param.name.as_deref().unwrap_or(name), item))
                        .collect::<Result<_, _>>()?,
                )
            }
        })
    }

    /// The value as a person reads it: decimal integers, `0x` hex, and the
    /// containers in brackets, a tuple's fields named where `kind` names them.
    #[must_use]
    pub fn show(&self, kind: &Kind) -> String {
        match (self, kind) {
            (Token::Uint(value), _) => value.to_string(),
            (Token::Int(value), _) => signed(*value),
            (Token::Address(address), _) => address.to_hex(),
            (Token::Bool(value), _) => value.to_string(),
            (Token::String(text), _) => text.clone(),
            (Token::Bytes(bytes) | Token::Fixed(bytes), _) => hex::encode_prefixed(bytes),
            (Token::Array(items), Kind::Array(inner) | Kind::Sized(inner, _)) => {
                format!("[{}]", items.iter().map(|item| item.show(inner)).collect::<Vec<_>>().join(", "))
            }
            (Token::Tuple(items), Kind::Tuple(params)) => format!(
                "{{{}}}",
                items
                    .iter()
                    .zip(params)
                    .map(|(item, param)| match &param.name {
                        Some(name) => format!("{name}: {}", item.show(&param.kind)),
                        None => item.show(&param.kind),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            (Token::Array(items) | Token::Tuple(items), _) => {
                format!("[{}]", items.iter().map(|item| item.show(kind)).collect::<Vec<_>>().join(", "))
            }
        }
    }

    /// A short name of the value, for a refusal.
    pub(super) fn describe(&self) -> String {
        match self {
            Token::Uint(value) => format!("the number {value}"),
            Token::Int(value) => format!("the number {}", signed(*value)),
            Token::Address(address) => format!("the address {address}"),
            Token::Bool(value) => format!("{value}"),
            Token::String(_) => "a string".to_owned(),
            Token::Bytes(bytes) => format!("{} bytes", bytes.len()),
            Token::Fixed(bytes) => format!("{} fixed bytes", bytes.len()),
            Token::Array(items) => format!("a list of {}", items.len()),
            Token::Tuple(items) => format!("a tuple of {}", items.len()),
        }
    }
}

fn power_of_two(exponent: u32) -> U256 {
    let mut word = [0u8; WORD];
    let at = WORD - 1 - (exponent / 8) as usize;
    word[at] = 1 << (exponent % 8);
    U256::from_be_bytes(word)
}

/// A two's complement word in decimal, with its sign.
fn signed(value: U256) -> String {
    match value.bits() == 256 {
        true => format!("-{}", negated(value)),
        false => value.to_string(),
    }
}

fn hex_of(text: &str) -> Option<Vec<u8>> {
    text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))?;
    hex::decode(text).ok()
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
