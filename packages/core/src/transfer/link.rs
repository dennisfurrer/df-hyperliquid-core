//! HyperCore and HyperEVM, linked: a linked spot token crosses between Core and EVM spot through
//! its system address. Core to EVM is a `spotSend` or `sendAsset` to [`Linked::system`]; EVM to
//! Core is a HyperEVM transaction to the same address ([`Linked::to_core`]).

use crate::client::evm::chain::NATIVE_DECIMALS;
use crate::client::evm::tx::Call;
use crate::client::evm::u256::{BadAmount, U256};
use crate::transfer::erc20;
use crate::utils::{Address, Dec};

/// Where a token crosses.
pub use crate::transfer::system::{HYPE_SYSTEM, is_system_address, system_address};
pub use crate::transfer::system_token;

/// A spot token as it is linked to HyperEVM. Its contract can hold arbitrary code; nothing here
/// checks its supply or what its `transfer` does.
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

    /// Where the token crosses, both ways. Sent there on Core, it arrives at the sender's address
    /// on HyperEVM.
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

    /// The HyperEVM call that sends `amount` back to Core: HYPE as a plain value transfer, any
    /// other token as an ERC-20 `transfer` to the system address. Exact, positive, and no finer
    /// than [`Linked::core_places`].
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
