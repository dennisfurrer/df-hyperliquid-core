//! `vaultDetails`, `userVaultEquities` and `leadingVaults`. `vaultDetails`
//! writes a few figures as bare JSON floats (`apr`, `maxWithdrawable`); they
//! are read as the digits sent ([`Figure`]), never through an f64. Its
//! `portfolio` history is left unread.

use crate::client::{ApiError, Transport, info, info_request, user_request};
use crate::earn::vaults::transfer::USD_DECIMALS;
use crate::utils::{Address, Dec, Figure, Json, Round};
use serde::Deserialize;

/// `{"type":"vaultDetails","vaultAddress":..,"user"?:..}`: with `user`, the
/// answer carries that follower's state.
#[must_use]
pub fn vault_details_request(vault: Address, user: Option<Address>) -> String {
    let mut fields = vec![("vaultAddress", Json::String(vault.to_hex()))];
    if let Some(user) = user {
        fields.push(("user", Json::String(user.to_hex())));
    }
    info_request("vaultDetails", &fields)
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
    /// Whether it leads strategy vaults (HLP), is one, or neither.
    #[serde(default)]
    pub relationship: Relationship,
    #[serde(default)]
    pub allow_deposits: bool,
    #[serde(default)]
    pub always_close_on_withdraw: bool,
}

/// `{"type":"parent","data":{"childAddresses":[..]}}`, `{"type":"child"}` or
/// `{"type":"normal"}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum Relationship {
    /// A vault anyone may follow, led by a person.
    #[default]
    Normal,
    /// Leads the strategy vaults it lists (HLP).
    #[serde(rename_all = "camelCase")]
    Parent { child_addresses: Vec<Address> },
    /// One of a parent's strategies, led by the parent itself.
    Child,
}

impl VaultDetails {
    /// Whether a deposit would be taken: open, and taking deposits.
    #[must_use]
    pub const fn takes_deposits(&self) -> bool {
        self.allow_deposits && !self.is_closed
    }

    /// What the asking follower may take out at `now_ms`, in USDC cut to
    /// millionths: its `maxWithdrawable` once its lockup has passed, nothing
    /// before, and nothing when no follower was asked about.
    #[must_use]
    pub fn withdrawable_now(&self, now_ms: u64) -> Dec {
        match &self.follower_state {
            Some(follower) if !follower.is_locked(now_ms) => self
                .max_withdrawable
                .as_ref()
                .and_then(Figure::to_dec)
                .map_or(Dec::ZERO, |most| most.round(USD_DECIMALS, Round::Down).max(Dec::ZERO)),
            _ => Dec::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Follower {
    /// `None` for the leader's own row, which the venue writes as `"user":"Leader"`.
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

impl Follower {
    /// Whether a withdrawal at `now_ms` would be refused for the lockup.
    #[must_use]
    pub fn is_locked(&self, now_ms: u64) -> bool {
        self.lockup_until.is_some_and(|until| now_ms < until)
    }
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

    /// What may leave at `now_ms`, in USDC: the equity cut to millionths once
    /// the lockup has passed, nothing before.
    #[must_use]
    pub fn withdrawable(&self, now_ms: u64) -> Dec {
        match self.is_locked(now_ms) {
            true => Dec::ZERO,
            false => self.equity.round(USD_DECIMALS, Round::Down).max(Dec::ZERO),
        }
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

#[cfg(test)]
#[path = "tests/details.rs"]
mod recorded;
