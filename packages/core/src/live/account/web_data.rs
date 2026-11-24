//! `webData3`: the account as Hyperliquid's own frontend reads it, its user state and each perp
//! dex's. Only the fields the docs list, and the account mode, are read.

use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `webData3`: the account as Hyperliquid's own frontend reads it. The docs warn that fields they do
/// not list may go; only the listed ones and `abstraction` are read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct WebData3 {
    /// `userState`: the account's own fields.
    pub user_state: UserState,
    /// `perpDexStates`: one entry per perp dex, in the order the dexes are listed.
    pub perp_dex_states: Vec<PerpDexState>,
}

/// `userState` in `webData3`: the account's own fields.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UserState {
    /// The account.
    pub user: Address,
    /// `agentAddress`: the agent wallet the frontend signs with; `None` when there is none.
    #[serde(default)]
    pub agent_address: Option<Address>,
    /// Milliseconds.
    #[serde(default)]
    pub agent_valid_until: Option<u64>,
    /// Milliseconds.
    pub server_time: u64,
    /// `cumLedger`: the account's cumulative ledger total, USDC.
    pub cum_ledger: Dec,
    /// `isVault`: whether the account is a vault.
    pub is_vault: bool,
    /// The account mode: `disabled`, `default`, `unifiedAccount`, `portfolioMargin`.
    #[serde(default)]
    pub abstraction: Option<String>,
    /// `dexAbstractionEnabled`: whether dex abstraction is on for the account; false when not sent.
    #[serde(default)]
    pub dex_abstraction_enabled: bool,
    /// `optOutOfSpotDusting`: whether the account opted out of spot dusting, the venue's sweep of
    /// tiny spot balances; false when not sent.
    #[serde(default)]
    pub opt_out_of_spot_dusting: bool,
}

/// One perp dex, in the order the dexes are listed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PerpDexState {
    /// `totalVaultEquity`: the account's equity in vaults on this dex, USDC.
    pub total_vault_equity: Dec,
    /// `perpsAtOpenInterestCap`: the perps on this dex at their open interest cap; empty when not sent.
    #[serde(default)]
    pub perps_at_open_interest_cap: Vec<String>,
    /// `leadingVaults`: the vaults the account leads on this dex; empty when not sent.
    #[serde(default)]
    pub leading_vaults: Vec<LeadingVault>,
}

/// A vault the account leads, in `leadingVaults`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct LeadingVault {
    /// The vault's address.
    pub address: Address,
    /// The vault's name.
    pub name: String,
}

#[cfg(test)]
#[path = "tests/web_data.rs"]
mod tests;
