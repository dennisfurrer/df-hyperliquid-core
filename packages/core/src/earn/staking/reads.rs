//! `validatorSummaries`, `delegatorSummary`, `delegations`,
//! `delegatorRewards`, `delegatorHistory`. HYPE figures are decimal strings,
//! except a validator's `stake`, which is wei.

use crate::client::{ApiError, Network, Transport, info, info_request, user_request};
use crate::utils::{Address, Dec, HYPE_DECIMALS};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A mainnet delegation is locked one day before it can be undelegated.
pub const MAINNET_DELEGATION_LOCK_MS: u64 = 86_400_000;

/// Testnet's delegation lock: two minutes, as live testnet delegations show.
pub const TESTNET_DELEGATION_LOCK_MS: u64 = 120_000;

/// A mainnet withdrawal from staking waits seven days in the queue before it
/// reaches spot.
pub const MAINNET_UNSTAKING_MS: u64 = 7 * 86_400_000;

/// Testnet's unstaking queue: five minutes, as live testnet histories show.
pub const TESTNET_UNSTAKING_MS: u64 = 300_000;

/// At most this many withdrawals wait in the queue at once.
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

/// `{"type":"delegatorSummary","user":..}`.
#[must_use]
pub fn delegator_summary_request(user: Address) -> String {
    user_request("delegatorSummary", user)
}

/// `{"type":"delegations","user":..}`.
#[must_use]
pub fn delegations_request(user: Address) -> String {
    user_request("delegations", user)
}

/// `{"type":"delegatorRewards","user":..}`.
#[must_use]
pub fn delegator_rewards_request(user: Address) -> String {
    user_request("delegatorRewards", user)
}

/// `{"type":"delegatorHistory","user":..}`.
#[must_use]
pub fn delegator_history_request(user: Address) -> String {
    user_request("delegatorHistory", user)
}

/// Every validator's summary.
pub async fn validator_summaries(transport: &impl Transport) -> Result<Vec<ValidatorSummary>, ApiError> {
    info(transport, validator_summaries_request()).await
}

/// The account's staking totals.
pub async fn delegator_summary(transport: &impl Transport, user: Address) -> Result<DelegatorSummary, ApiError> {
    info(transport, delegator_summary_request(user)).await
}

/// The account's delegations, one per validator.
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

/// One validator, as `validatorSummaries` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorSummary {
    /// The validator's address.
    pub validator: Address,
    /// The address that signs the validator's blocks.
    pub signer: Address,
    /// The validator's name; empty when the venue leaves it out.
    #[serde(default)]
    pub name: String,
    /// The validator's description; empty when the venue leaves it out.
    #[serde(default)]
    pub description: String,
    /// How many blocks the validator produced recently (`nRecentBlocks`).
    #[serde(default)]
    pub n_recent_blocks: u64,
    /// In wei, 1e8 to a HYPE.
    pub stake: u64,
    /// Whether the validator is jailed (`isJailed`) and earns no rewards.
    #[serde(default)]
    pub is_jailed: bool,
    /// When a jailed validator may unjail itself (`unjailableAfter`), milliseconds; `None` when not set.
    #[serde(default)]
    pub unjailable_after: Option<u64>,
    /// Whether the validator is in the active set (`isActive`).
    #[serde(default)]
    pub is_active: bool,
    /// A fraction of rewards: 0.04 is 4%.
    pub commission: Dec,
    /// By window: `day`, `week`, `month`.
    #[serde(default)]
    pub stats: Vec<(String, ValidatorStats)>,
}

/// A validator's figures over one window.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorStats {
    /// The share of the window the validator was up (`uptimeFraction`), as a fraction.
    pub uptime_fraction: Dec,
    /// Yearly, as a fraction.
    pub predicted_apr: Dec,
    /// How many samples the figures come from (`nSamples`).
    #[serde(default)]
    pub n_samples: u64,
}

impl ValidatorSummary {
    /// The stake in HYPE; `None` past what a [`Dec`] holds.
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

/// An account's staking totals, as `delegatorSummary` answers them. HYPE.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatorSummary {
    /// Everything delegated to validators.
    pub delegated: Dec,
    /// The staking balance free to delegate or withdraw.
    pub undelegated: Dec,
    /// HYPE in the unstaking queue (`totalPendingWithdrawal`).
    pub total_pending_withdrawal: Dec,
    /// How many withdrawals wait in the queue (`nPendingWithdrawals`).
    pub n_pending_withdrawals: u64,
}

/// One delegation, as `delegations` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
    /// The validator delegated to.
    pub validator: Address,
    /// The HYPE delegated.
    pub amount: Dec,
    /// Milliseconds; undelegating before it is refused.
    pub locked_until_timestamp: u64,
}

impl Delegation {
    /// Whether undelegating at `now_ms` is refused because the lock has not ended.
    #[must_use]
    pub const fn is_locked(&self, now_ms: u64) -> bool {
        now_ms < self.locked_until_timestamp
    }
}

/// One reward credit, as `delegatorRewards` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    /// When it was credited, milliseconds.
    pub time: u64,
    /// `delegation`, or `commission` for a validator's own cut.
    #[serde(default)]
    pub source: String,
    /// The HYPE credited (`totalAmount`).
    pub total_amount: Dec,
}

/// One row of `delegatorHistory`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StakeEvent {
    /// When it happened, milliseconds.
    pub time: u64,
    /// The L1 transaction hash.
    pub hash: String,
    /// What the row did.
    #[serde(deserialize_with = "delta")]
    pub delta: StakeDelta,
}

/// What a history row did, keyed by its one field's name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StakeDelta {
    /// `delegate`: staking balance to a validator, or back.
    #[serde(rename_all = "camelCase")]
    Delegate {
        /// The validator.
        validator: Address,
        /// The HYPE moved.
        amount: Dec,
        /// `true` for an undelegation (`isUndelegate`).
        is_undelegate: bool,
    },
    /// `withdrawal`: from staking toward spot: `initiated`, then `finalized` once queued
    /// [`unstaking_ms`].
    Withdrawal {
        /// The HYPE withdrawn.
        amount: Dec,
        /// `initiated` or `finalized`.
        phase: String,
    },
    /// `cDeposit`: spot into staking.
    CDeposit {
        /// The HYPE deposited.
        amount: Dec,
    },
    /// A kind this build does not read, by name.
    #[serde(skip_deserializing)]
    Other {
        /// The row's field name.
        kind: String,
    },
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
