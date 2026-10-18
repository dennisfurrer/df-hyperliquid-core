//! A precompile's answer decoded: the tokens against its declared return, and a
//! struct's fields read in order.

use crate::client::evm::abi::{self, AbiError, Token};
use crate::client::evm::precompiles::table::spec;
use crate::utils::Address;

/// The answer of the precompile at `at`, decoded against its declared return.
pub(super) fn tokens(at: u16, answer: &[u8]) -> Result<Vec<Token>, AbiError> {
    let returns = spec(at).map_or("", |one| one.returns);
    abi::decode(&abi::params(returns)?, answer)
}

/// The fields of a struct answer, read in order.
pub(super) struct Fields<'a> {
    name: &'static str,
    pub(super) items: std::slice::Iter<'a, Token>,
}

impl<'a> Fields<'a> {
    /// The fields of the one tuple an answer is.
    pub(super) fn of(name: &'static str, tokens: &'a [Token]) -> Result<Fields<'a>, AbiError> {
        match tokens {
            [Token::Tuple(items)] => Ok(Fields { name, items: items.iter() }),
            _ => Err(AbiError::Type(format!("{name}: not one struct"))),
        }
    }

    fn take<T>(&mut self, read: impl FnOnce(&'a Token) -> Option<T>) -> Result<T, AbiError> {
        self.items
            .next()
            .and_then(read)
            .ok_or_else(|| AbiError::Type(format!("{}: a field of another type", self.name)))
    }

    pub(super) fn u64(&mut self) -> Result<u64, AbiError> {
        self.take(Token::as_u64)
    }

    pub(super) fn u32(&mut self) -> Result<u32, AbiError> {
        self.take(Token::as_u32)
    }

    pub(super) fn u8(&mut self) -> Result<u8, AbiError> {
        self.take(Token::as_u8)
    }

    pub(super) fn i64(&mut self) -> Result<i64, AbiError> {
        self.take(Token::as_i64)
    }

    pub(super) fn i8(&mut self) -> Result<i8, AbiError> {
        self.take(Token::as_i8)
    }

    pub(super) fn bool(&mut self) -> Result<bool, AbiError> {
        self.take(Token::as_bool)
    }

    pub(super) fn address(&mut self) -> Result<Address, AbiError> {
        self.take(Token::as_address)
    }

    pub(super) fn string(&mut self) -> Result<String, AbiError> {
        self.take(|token| token.as_str().map(ToOwned::to_owned))
    }

    pub(super) fn items(&mut self) -> Result<&'a [Token], AbiError> {
        self.take(Token::as_items)
    }

    pub(super) fn u64s(&mut self) -> Result<Vec<u64>, AbiError> {
        let name = self.name;
        self.items()?
            .iter()
            .map(|item| item.as_u64().ok_or_else(|| AbiError::Type(format!("{name}: not a list of uint64"))))
            .collect()
    }
}

pub(super) fn bare(at: u16, answer: &[u8]) -> Result<u64, AbiError> {
    match tokens(at, answer)?.as_slice() {
        [one] => one.as_u64().ok_or_else(|| AbiError::Type("not a uint64".to_owned())),
        _ => Err(AbiError::Type("not one uint64".to_owned())),
    }
}

#[cfg(test)]
#[path = "tests/fields.rs"]
mod tests;
