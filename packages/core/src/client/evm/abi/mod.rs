//! Solidity's ABI encoding: integers to 256 bits, addresses, booleans, `bytesN`,
//! `bytes`, strings, arrays and nested tuples. Types are read from their
//! human-readable form, and every value and word is checked against its type.

pub mod decode;
pub mod encode;
pub mod error;
pub mod function;
pub mod kind;
mod text;
pub mod token;

pub use decode::decode;
pub use encode::encode;
pub use error::AbiError;
pub use function::{Function, event_topic, selector};
pub use kind::{Kind, Param, params, tuple_type};
pub use token::Token;

const WORD: usize = 32;

/// The most a length or offset inside the data is believed before it is refused.
const LONGEST: usize = 1 << 20;

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
