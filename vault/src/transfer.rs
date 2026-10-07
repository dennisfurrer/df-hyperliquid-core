//! `vaultTransfer`: `{type, vaultAddress, isDeposit, usd}`, `usd` in
//! millionths, keys in the `@nktkas/hyperliquid` 0.33.2 order.

use hl_core::{Address, BadUnits, Dec, L1Action, Map, exact_units};

/// The protocol vault, which any account can deposit into.
pub const HLP: Address = match Address::parse_const("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303") {
    Some(address) => address,
    None => Address::ZERO,
};

/// How long a deposit into HLP is locked: four days.
pub const HLP_LOCKUP_MS: u64 = 4 * 86_400_000;

/// How long a deposit into any other vault is locked: a day.
pub const VAULT_LOCKUP_MS: u64 = 86_400_000;

/// `usd` counts USDC in 1e6.
pub const USD_DECIMALS: u32 = 6;

/// The lockup a deposit into `vault` starts.
#[must_use]
pub fn lockup_ms(vault: Address) -> u64 {
    match vault == HLP {
        true => HLP_LOCKUP_MS,
        false => VAULT_LOCKUP_MS,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultTransfer {
    vault: Address,
    is_deposit: bool,
    usd: u64,
}

impl VaultTransfer {
    /// USDC from the perp balance into `vault`, at most six places: refused
    /// rather than rounded when finer.
    pub fn deposit(vault: Address, usdc: Dec) -> Result<VaultTransfer, BadUnits> {
        Ok(VaultTransfer { vault, is_deposit: true, usd: exact_units(usdc, USD_DECIMALS)? })
    }

    /// USDC out of `vault` once its lockup has passed. A whole equity runs
    /// past six places; cut it first (`hl_core::cut_units`) or take
    /// `maxWithdrawable` from [`crate::vault_details`].
    pub fn withdraw(vault: Address, usdc: Dec) -> Result<VaultTransfer, BadUnits> {
        Ok(VaultTransfer { vault, is_deposit: false, usd: exact_units(usdc, USD_DECIMALS)? })
    }

    /// From a count of millionths already made.
    #[must_use]
    pub const fn from_micros(vault: Address, is_deposit: bool, usd: u64) -> VaultTransfer {
        VaultTransfer { vault, is_deposit, usd }
    }

    #[must_use]
    pub const fn usd(&self) -> u64 {
        self.usd
    }
}

impl L1Action for VaultTransfer {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "vaultTransfer")
            .put("vaultAddress", self.vault.to_hex())
            .put("isDeposit", self.is_deposit)
            .put("usd", self.usd)
    }
}
