//! The ERC-20 calls a bridge or a Core link needs: `transfer` (USDC into the
//! Arbitrum bridge, a token back to Core through its system address),
//! `balanceOf`, `approve`, `decimals` and `symbol`, and their answers.

use crate::client::evm::abi::{self, AbiError, Token};
use crate::client::evm::u256::{B256, U256};
use crate::utils::Address;

/// `transfer(address,uint256)`.
pub const TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
/// `balanceOf(address)`.
pub const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
/// `approve(address,uint256)`.
pub const APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];
/// `decimals()`.
pub const DECIMALS: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67];
/// `symbol()`.
pub const SYMBOL: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41];

fn call(selector: [u8; 4], words: &[[u8; 32]]) -> Vec<u8> {
    let mut out = selector.to_vec();
    for word in words {
        out.extend_from_slice(word);
    }
    out
}

/// `transfer(to, amount)`, the amount in the token's own units.
#[must_use]
pub fn transfer(to: Address, amount: U256) -> Vec<u8> {
    call(TRANSFER, &[B256::from_address(to).0, amount.to_be_bytes()])
}

/// `balanceOf(owner)`; its answer reads with [`read_amount`].
#[must_use]
pub fn balance_of(owner: Address) -> Vec<u8> {
    call(BALANCE_OF, &[B256::from_address(owner).0])
}

/// `approve(spender, amount)`.
#[must_use]
pub fn approve(spender: Address, amount: U256) -> Vec<u8> {
    call(APPROVE, &[B256::from_address(spender).0, amount.to_be_bytes()])
}

/// `decimals()`; its answer reads with [`read_decimals`].
#[must_use]
pub fn decimals() -> Vec<u8> {
    DECIMALS.to_vec()
}

/// `symbol()`; its answer reads with [`read_symbol`].
#[must_use]
pub fn symbol() -> Vec<u8> {
    SYMBOL.to_vec()
}

fn one(shape: &str, answer: &[u8]) -> Result<Token, AbiError> {
    abi::decode(&abi::params(shape)?, answer)?.into_iter().next().ok_or(AbiError::Short(answer.len()))
}

/// A `uint256` answer: a balance or an allowance.
pub fn read_amount(answer: &[u8]) -> Result<U256, AbiError> {
    one("uint256", answer)?.as_u256().ok_or(AbiError::Short(answer.len()))
}

/// The `uint8` `decimals()` answers.
pub fn read_decimals(answer: &[u8]) -> Result<u8, AbiError> {
    one("uint8", answer)?.as_u8().ok_or(AbiError::Short(answer.len()))
}

/// The `string` `symbol()` answers.
pub fn read_symbol(answer: &[u8]) -> Result<String, AbiError> {
    match one("string", answer)? {
        Token::String(text) => Ok(text),
        _ => Err(AbiError::Short(answer.len())),
    }
}

#[cfg(test)]
#[path = "tests/erc20.rs"]
mod tests;
