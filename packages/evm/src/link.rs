//! HyperCore and HyperEVM, linked: every spot token has a system address,
//! and a linked token crosses between Core spot and EVM spot through it
//! (the docs' `hypercore-less-than-greater-than-hyperevm-transfers`).
//!
//! - **Core to EVM** is a `spotSend` or `sendAsset` on Core to the token's
//!   system address; the token arrives at the sender's address on HyperEVM.
//!   That action is signed on Core (`hl-transfer`); here is where it goes,
//!   [`Linked::system`], and how fine the amount may be, [`Linked::core_places`].
//! - **EVM to Core** is a HyperEVM transaction to the same system address:
//!   HYPE as a plain value transfer, any other token as an ERC-20
//!   `transfer(system address, amount)` on its contract ([`Linked::to_core`]).
//!
//! A linked contract can hold arbitrary code; nothing checks its supply or
//! what its `transfer` does. A value with more EVM decimals than Core keeps
//! loses the remainder, less than one Core wei.

use hl_core::{Address, Dec};

use crate::{
    chain::NATIVE_DECIMALS,
    erc20,
    tx::Call,
    u256::{BadAmount, U256},
};

/// HYPE's system address: HYPE is the native coin on HyperEVM.
pub const HYPE_SYSTEM: Address = Address::from_bytes([0x22; 20]);

/// `0x20`, then the token index big-endian in the low bytes: token 200 is
/// `0x2000…00c8`.
#[must_use]
pub fn system_address(index: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[0] = 0x20;
    bytes[12..].copy_from_slice(&index.to_be_bytes());
    Address::from_bytes(bytes)
}

/// Whether a send to `address` crosses between Core and HyperEVM rather than
/// reaching a person.
#[must_use]
pub fn is_system_address(address: Address) -> bool {
    address == HYPE_SYSTEM || system_token(address).is_some()
}

/// The token index a `0x20` system address stands for.
#[must_use]
pub fn system_token(address: Address) -> Option<u64> {
    let bytes = address.bytes();
    if bytes[0] != 0x20 || bytes[1..12].iter().any(|byte| *byte != 0) {
        return None;
    }
    let mut index = [0u8; 8];
    index.copy_from_slice(&bytes[12..]);
    Some(u64::from_be_bytes(index))
}

/// A spot token as it is linked to HyperEVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linked {
    pub index: u64,
    /// The ERC-20; `None` for HYPE, which is the chain's native coin.
    pub contract: Option<Address>,
    /// The decimals Core counts the token in (`spotMeta`'s `weiDecimals`).
    pub wei_decimals: u32,
    /// How many more the ERC-20 counts in, -2 to 18.
    pub evm_extra_wei_decimals: i32,
}

impl Linked {
    /// HYPE: native on HyperEVM, eighteen decimals there.
    #[must_use]
    pub const fn hype(wei_decimals: u32) -> Linked {
        Linked { index: 0, contract: None, wei_decimals, evm_extra_wei_decimals: 0 }
    }

    /// A token linked to an ERC-20 (`spotMeta`'s `evmContract`).
    #[must_use]
    pub const fn erc20(index: u64, contract: Address, wei_decimals: u32, evm_extra_wei_decimals: i32) -> Linked {
        Linked { index, contract: Some(contract), wei_decimals, evm_extra_wei_decimals }
    }

    /// Where the token crosses, both ways.
    #[must_use]
    pub fn system(&self) -> Address {
        match self.contract {
            None => HYPE_SYSTEM,
            Some(_) => system_address(self.index),
        }
    }

    /// The decimals the token counts in on HyperEVM.
    #[must_use]
    pub fn evm_decimals(&self) -> Option<u32> {
        match self.contract {
            None => Some(NATIVE_DECIMALS),
            Some(_) => u32::try_from(i64::from(self.wei_decimals) + i64::from(self.evm_extra_wei_decimals)).ok(),
        }
    }

    /// The most places an amount crossing either way may have: the fewer of
    /// Core's and the EVM's.
    #[must_use]
    pub fn core_places(&self) -> Option<u32> {
        self.evm_decimals().map(|evm| evm.min(self.wei_decimals))
    }

    /// The HyperEVM call that sends `amount` back to Core: exact, positive,
    /// and no finer than [`Linked::core_places`].
    pub fn to_core(&self, amount: Dec) -> Result<Call, BadAmount> {
        let places = self.core_places().ok_or(BadAmount::TooLarge)?;
        let decimals = self.evm_decimals().ok_or(BadAmount::TooLarge)?;
        if !amount.is_positive() {
            return Err(BadAmount::NotPositive(amount));
        }
        if amount.places() > places {
            return Err(BadAmount::TooFine { value: amount, decimals: places });
        }
        let units = U256::from_dec(amount, decimals)?;
        Ok(match self.contract {
            None => Call { to: Some(HYPE_SYSTEM), value: units, data: Vec::new() },
            Some(contract) => {
                Call { to: Some(contract), value: U256::ZERO, data: erc20::transfer(self.system(), units) }
            }
        })
    }
}

#[cfg(test)]
#[path = "tests/link.rs"]
mod tests;
