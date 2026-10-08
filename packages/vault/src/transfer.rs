//! `vaultTransfer`: `{type, vaultAddress, isDeposit, usd}`, `usd` in
//! millionths, keys in the `@nktkas/hyperliquid` 0.33.2 order. HLP's address
//! and a vault's lockup differ between the networks.

use hl_core::{Address, BadUnits, Dec, L1Action, Map, Network, exact_units};

use crate::plan::Key;

const fn address(text: &str) -> Address {
    match Address::parse_const(text) {
        Some(address) => address,
        None => Address::ZERO,
    }
}

/// The protocol vault on mainnet, which any account can deposit into.
pub const MAINNET_HLP: Address = address("0xdfc24b077bc1425ad1dea75bcb6f8158e10df303");

/// Testnet's protocol vault, as its `vaultDetails` names it ("Hyperliquidity
/// Provider (HLP)", read on 2026-10-08). hip4-tui found it unpublished.
pub const TESTNET_HLP: Address = address("0xa15099a30bbf2e68942d6f4c43d70d04faeab0a0");

/// How long a deposit into HLP is locked, on both networks: four days, as
/// every single-deposit follower of either shows.
pub const HLP_LOCKUP_MS: u64 = 4 * 86_400_000;

/// How long a deposit into any other vault on mainnet is locked: a day.
pub const MAINNET_VAULT_LOCKUP_MS: u64 = 86_400_000;

/// Testnet's: twenty seconds, as the single-deposit followers of its user
/// vaults show, from early 2025 to September 2026.
pub const TESTNET_VAULT_LOCKUP_MS: u64 = 20_000;

/// `usd` counts USDC in 1e6.
pub const USD_DECIMALS: u32 = 6;

/// The network's HLP.
#[must_use]
pub const fn hlp(network: Network) -> Address {
    match network {
        Network::Mainnet => MAINNET_HLP,
        Network::Testnet => TESTNET_HLP,
    }
}

/// The lockup a deposit into `vault` on `network` starts.
#[must_use]
pub fn lockup_ms(network: Network, vault: Address) -> u64 {
    match (vault == hlp(network), network) {
        (true, _) => HLP_LOCKUP_MS,
        (false, Network::Mainnet) => MAINNET_VAULT_LOCKUP_MS,
        (false, Network::Testnet) => TESTNET_VAULT_LOCKUP_MS,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultTransfer {
    vault: Address,
    is_deposit: bool,
    usd: u64,
}

impl VaultTransfer {
    /// L1: an agent may sign it for its master (hip4-tui docs/WIRE.md).
    pub const KEY: Key = Key::Either;

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

    #[must_use]
    pub const fn vault(&self) -> Address {
        self.vault
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
