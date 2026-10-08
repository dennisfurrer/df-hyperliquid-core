//! `validatorSummaries`, `delegatorSummary`, `delegations`,
//! `delegatorRewards`, `delegatorHistory`. HYPE figures are decimal strings,
//! except a validator's `stake`, which is wei.

use hl_core::{Address, ApiError, Dec, Network, Transport, info, info_request, user_request};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::actions::HYPE_DECIMALS;

/// A delegation on mainnet is locked a day before it can be undelegated (the
/// venue's staking docs); each live lock ends a day after its delegation.
pub const MAINNET_DELEGATION_LOCK_MS: u64 = 86_400_000;

/// Testnet's lock: two minutes, as every live testnet delegation read on
/// 2026-10-08 shows.
pub const TESTNET_DELEGATION_LOCK_MS: u64 = 120_000;

/// A withdrawal from staking to spot on mainnet waits seven days in the queue
/// (the venue's staking docs). A live history read 604800145 ms from
/// `initiated` to `finalized`.
pub const MAINNET_UNSTAKING_MS: u64 = 7 * 86_400_000;

/// Testnet's queue: five minutes. Live testnet histories read 300133 to
/// 300996 ms from `initiated` to `finalized`.
pub const TESTNET_UNSTAKING_MS: u64 = 300_000;

/// At most this many withdrawals wait in the queue at once (the venue's
/// staking docs).
pub const MAX_PENDING_WITHDRAWALS: u64 = 5;

/// How long a new delegation on `network` is locked.
#[must_use]
pub const fn delegation_lock_ms(network: Network) -> u64 {
    match network {
        Network::Mainnet => MAINNET_DELEGATION_LOCK_MS,
        Network::Testnet => TESTNET_DELEGATION_LOCK_MS,
    }
}

/// How long a withdrawal on `network` waits before it reaches spot.
#[must_use]
pub const fn unstaking_ms(network: Network) -> u64 {
    match network {
        Network::Mainnet => MAINNET_UNSTAKING_MS,
        Network::Testnet => TESTNET_UNSTAKING_MS,
    }
}

/// When a delegation made at `now_ms` may first be undelegated.
#[must_use]
pub const fn locked_until(network: Network, now_ms: u64) -> u64 {
    now_ms.saturating_add(delegation_lock_ms(network))
}

/// `{"type":"validatorSummaries"}`.
#[must_use]
pub fn validator_summaries_request() -> String {
    info_request("validatorSummaries", &[])
}

#[must_use]
pub fn delegator_summary_request(user: Address) -> String {
    user_request("delegatorSummary", user)
}

#[must_use]
pub fn delegations_request(user: Address) -> String {
    user_request("delegations", user)
}

#[must_use]
pub fn delegator_rewards_request(user: Address) -> String {
    user_request("delegatorRewards", user)
}

#[must_use]
pub fn delegator_history_request(user: Address) -> String {
    user_request("delegatorHistory", user)
}

pub async fn validator_summaries(transport: &impl Transport) -> Result<Vec<ValidatorSummary>, ApiError> {
    info(transport, validator_summaries_request()).await
}

pub async fn delegator_summary(transport: &impl Transport, user: Address) -> Result<DelegatorSummary, ApiError> {
    info(transport, delegator_summary_request(user)).await
}

pub async fn delegations(transport: &impl Transport, user: Address) -> Result<Vec<Delegation>, ApiError> {
    info(transport, delegations_request(user)).await
}

/// Newest first.
pub async fn delegator_rewards(transport: &impl Transport, user: Address) -> Result<Vec<Reward>, ApiError> {
    info(transport, delegator_rewards_request(user)).await
}

/// Newest first.
pub async fn delegator_history(transport: &impl Transport, user: Address) -> Result<Vec<StakeEvent>, ApiError> {
    info(transport, delegator_history_request(user)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorSummary {
    pub validator: Address,
    pub signer: Address,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub n_recent_blocks: u64,
    /// In wei, 1e8 to a HYPE.
    pub stake: u64,
    #[serde(default)]
    pub is_jailed: bool,
    #[serde(default)]
    pub unjailable_after: Option<u64>,
    #[serde(default)]
    pub is_active: bool,
    /// A fraction of rewards: 0.04 is 4%.
    pub commission: Dec,
    /// By window: `day`, `week`, `month`.
    #[serde(default)]
    pub stats: Vec<(String, ValidatorStats)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorStats {
    pub uptime_fraction: Dec,
    /// Yearly, as a fraction.
    pub predicted_apr: Dec,
    #[serde(default)]
    pub n_samples: u64,
}

impl ValidatorSummary {
    #[must_use]
    pub fn stake_hype(&self) -> Option<Dec> {
        Dec::new(i128::from(self.stake), HYPE_DECIMALS)
    }

    /// The stats over one window: `week` is what most clients show.
    #[must_use]
    pub fn stats(&self, window: &str) -> Option<&ValidatorStats> {
        self.stats.iter().find(|(name, _)| name == window).map(|(_, stats)| stats)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatorSummary {
    pub delegated: Dec,
    /// The staking balance free to delegate or withdraw.
    pub undelegated: Dec,
    pub total_pending_withdrawal: Dec,
    pub n_pending_withdrawals: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
    pub validator: Address,
    pub amount: Dec,
    /// Milliseconds; undelegating before it is refused.
    pub locked_until_timestamp: u64,
}

impl Delegation {
    #[must_use]
    pub const fn is_locked(&self, now_ms: u64) -> bool {
        now_ms < self.locked_until_timestamp
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub time: u64,
    /// `delegation`, or `commission` for a validator's own cut.
    #[serde(default)]
    pub source: String,
    pub total_amount: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StakeEvent {
    pub time: u64,
    pub hash: String,
    #[serde(deserialize_with = "delta")]
    pub delta: StakeDelta,
}

/// What a history row did, keyed by its one field's name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StakeDelta {
    #[serde(rename_all = "camelCase")]
    Delegate { validator: Address, amount: Dec, is_undelegate: bool },
    /// From staking toward spot: `initiated`, then `finalized` once queued
    /// [`unstaking_ms`].
    Withdrawal { amount: Dec, phase: String },
    /// Spot into staking.
    CDeposit { amount: Dec },
    /// A kind this build does not read, by name.
    #[serde(skip_deserializing)]
    Other { kind: String },
}

impl StakeEvent {
    /// When an initiated withdrawal reaches spot on `network`.
    #[must_use]
    pub fn unstaked_at(&self, network: Network) -> Option<u64> {
        match &self.delta {
            StakeDelta::Withdrawal { phase, .. } if phase == "initiated" => {
                Some(self.time.saturating_add(unstaking_ms(network)))
            }
            _ => None,
        }
    }
}

/// A known kind that no longer reads as modelled falls back to `Other` too:
/// one changed row must not blank the history.
fn delta<'de, D: Deserializer<'de>>(deserializer: D) -> Result<StakeDelta, D::Error> {
    let value = Value::deserialize(deserializer)?;
    let kind = value.as_object().and_then(|object| object.keys().next().cloned()).unwrap_or_default();
    Ok(serde_json::from_value(value).unwrap_or(StakeDelta::Other { kind }))
}

#[cfg(test)]
#[path = "tests/stake.rs"]
mod tests;
