//! `webData3`: the account as Hyperliquid's own frontend reads it, its user state and each perp
//! dex's. Only the fields the docs list, and the account mode, are read.

use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `webData3`: the account as Hyperliquid's own frontend reads it. The docs warn that fields they do
/// not list may go; only the listed ones and `abstraction` are read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebData3 {
    pub user_state: UserState,
    pub perp_dex_states: Vec<PerpDexState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    pub user: Address,
    #[serde(default)]
    pub agent_address: Option<Address>,
    /// Milliseconds.
    #[serde(default)]
    pub agent_valid_until: Option<u64>,
    /// Milliseconds.
    pub server_time: u64,
    pub cum_ledger: Dec,
    pub is_vault: bool,
    /// The account mode: `disabled`, `default`, `unifiedAccount`, `portfolioMargin`.
    #[serde(default)]
    pub abstraction: Option<String>,
    #[serde(default)]
    pub dex_abstraction_enabled: bool,
    #[serde(default)]
    pub opt_out_of_spot_dusting: bool,
}

/// One perp dex, in the order the dexes are listed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDexState {
    pub total_vault_equity: Dec,
    #[serde(default)]
    pub perps_at_open_interest_cap: Vec<String>,
    #[serde(default)]
    pub leading_vaults: Vec<LeadingVault>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LeadingVault {
    pub address: Address,
    pub name: String,
}

#[cfg(test)]
#[path = "tests/web_data.rs"]
mod tests;
