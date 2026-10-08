//! `vaultDetails`, `userVaultEquities` and `leadingVaults`. `vaultDetails`
//! writes a few figures as bare JSON floats (`apr`, `maxWithdrawable`); they
//! are read as the digits sent ([`Figure`]), never through an f64. Its
//! `portfolio` history is left unread.

use hl_core::{Address, ApiError, Dec, Figure, Json, Transport, info, user_request};
use serde::Deserialize;

/// `{"type":"vaultDetails","vaultAddress":..,"user"?:..}`: with `user`, the
/// answer carries that follower's state.
#[must_use]
pub fn vault_details_request(vault: Address, user: Option<Address>) -> String {
    let mut fields = vec![
        ("type".to_owned(), Json::String("vaultDetails".to_owned())),
        ("vaultAddress".to_owned(), Json::String(vault.to_hex())),
    ];
    if let Some(user) = user {
        fields.push(("user".to_owned(), Json::String(user.to_hex())));
    }
    Json::Object(fields).compact()
}

#[must_use]
pub fn user_vault_equities_request(user: Address) -> String {
    user_request("userVaultEquities", user)
}

#[must_use]
pub fn leading_vaults_request(user: Address) -> String {
    user_request("leadingVaults", user)
}

/// `None` for an address that is no vault: the venue answers `null`.
pub async fn vault_details(
    transport: &impl Transport,
    vault: Address,
    user: Option<Address>,
) -> Result<Option<VaultDetails>, ApiError> {
    info(transport, vault_details_request(vault, user)).await
}

/// Every vault `user` has equity in, with when each unlocks.
pub async fn user_vault_equities(transport: &impl Transport, user: Address) -> Result<Vec<VaultEquity>, ApiError> {
    info(transport, user_vault_equities_request(user)).await
}

/// The vaults `user` leads.
pub async fn leading_vaults(transport: &impl Transport, user: Address) -> Result<Vec<LeadingVault>, ApiError> {
    info(transport, leading_vaults_request(user)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultDetails {
    pub name: String,
    pub vault_address: Address,
    pub leader: Address,
    #[serde(default)]
    pub description: String,
    /// Yearly, as a fraction.
    #[serde(default)]
    pub apr: Option<Figure>,
    /// The asking follower's state, when `user` was named and follows.
    #[serde(default)]
    pub follower_state: Option<Follower>,
    /// The leader's share of the vault.
    #[serde(default)]
    pub leader_fraction: Option<Figure>,
    /// The leader's cut of followers' profit.
    #[serde(default)]
    pub leader_commission: Option<Figure>,
    #[serde(default)]
    pub followers: Vec<Follower>,
    #[serde(default)]
    pub max_distributable: Option<Figure>,
    /// For the asking follower: what may leave now, in USDC.
    #[serde(default)]
    pub max_withdrawable: Option<Figure>,
    #[serde(default)]
    pub is_closed: bool,
    #[serde(default)]
    pub allow_deposits: bool,
    #[serde(default)]
    pub always_close_on_withdraw: bool,
}

impl VaultDetails {
    /// Whether a deposit would be taken: open, and taking deposits.
    #[must_use]
    pub const fn takes_deposits(&self) -> bool {
        self.allow_deposits && !self.is_closed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Follower {
    /// None for the leader's own row, which the venue writes as `"user":"Leader"` (seen in HLP's
    /// followers on 2026-10-08).
    #[serde(deserialize_with = "follower_user")]
    pub user: Option<Address>,
    pub vault_equity: Dec,
    #[serde(default)]
    pub pnl: Option<Dec>,
    #[serde(default)]
    pub all_time_pnl: Option<Dec>,
    #[serde(default)]
    pub days_following: Option<u64>,
    #[serde(default)]
    pub vault_entry_time: Option<u64>,
    /// Milliseconds; a withdrawal before it is refused.
    #[serde(default)]
    pub lockup_until: Option<u64>,
}

/// A follower's `user`: an address, or `Leader` for the leader's own stake.
fn follower_user<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Address>, D::Error> {
    let text = String::deserialize(deserializer)?;
    if text == "Leader" {
        return Ok(None);
    }
    text.parse().map(Some).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEquity {
    pub vault_address: Address,
    pub equity: Dec,
    /// Milliseconds.
    pub locked_until_timestamp: u64,
}

impl VaultEquity {
    #[must_use]
    pub const fn is_locked(&self, now_ms: u64) -> bool {
        now_ms < self.locked_until_timestamp
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LeadingVault {
    pub address: Address,
    #[serde(default)]
    pub name: String,
}

#[cfg(test)]
#[path = "tests/vault.rs"]
mod tests;
