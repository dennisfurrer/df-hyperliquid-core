//! An account's whole stake from its four reads: the summary, each
//! delegation, the withdrawals still in the queue (dated from the history),
//! and a month's rewards summed exactly.

use crate::client::{ApiError, Network, Transport};
use crate::earn::staking::reads::{
    Delegation, DelegatorSummary, Reward, StakeDelta, StakeEvent, delegations, delegator_history, delegator_rewards,
    delegator_summary,
};
use crate::utils::{Address, Dec};

/// The window [`Staking::rewards`] sums: thirty days.
pub const REWARDS_WINDOW_MS: u64 = 30 * 86_400_000;

/// A withdrawal from staking still on its way to spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingWithdrawal {
    /// The HYPE withdrawn.
    pub amount: Dec,
    /// When it was asked for, milliseconds.
    pub initiated: u64,
    /// When it reaches spot.
    pub ready_at: u64,
}

/// Withdrawals asked for by `now_ms` whose queue has not ended, newest first
/// as the history lists them.
#[must_use]
pub fn pending_withdrawals(network: Network, history: &[StakeEvent], now_ms: u64) -> Vec<PendingWithdrawal> {
    history
        .iter()
        .filter(|event| event.time <= now_ms)
        .filter_map(|event| {
            let StakeDelta::Withdrawal { amount, .. } = &event.delta else { return None };
            let ready_at = event.unstaked_at(network)?;
            (ready_at > now_ms).then_some(PendingWithdrawal { amount: *amount, initiated: event.time, ready_at })
        })
        .collect()
}

/// Every reward credited at or after `since_ms`, summed exactly. `None` past
/// what a [`Dec`] holds.
#[must_use]
pub fn rewards_since(rewards: &[Reward], since_ms: u64) -> Option<Dec> {
    Dec::checked_sum(rewards.iter().filter(|reward| reward.time >= since_ms).map(|reward| reward.total_amount))
}

/// One account's stake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staking {
    /// The account's `delegatorSummary`.
    pub summary: DelegatorSummary,
    /// Each delegation, as `delegations` lists them.
    pub delegations: Vec<Delegation>,
    /// The newest withdrawals still queued, as many as the summary counts.
    pub pending: Vec<PendingWithdrawal>,
    /// Rewards over the last [`REWARDS_WINDOW_MS`].
    pub rewards: Dec,
}

impl Staking {
    /// Puts the reads together at `now_ms`: keeps as many queued withdrawals as the summary counts and
    /// sums the rewards over the last [`REWARDS_WINDOW_MS`].
    #[must_use]
    pub fn new(
        network: Network,
        summary: DelegatorSummary,
        delegations: Vec<Delegation>,
        rewards: &[Reward],
        history: &[StakeEvent],
        now_ms: u64,
    ) -> Staking {
        let count = usize::try_from(summary.n_pending_withdrawals).unwrap_or(usize::MAX);
        let mut pending = pending_withdrawals(network, history, now_ms);
        pending.truncate(count);
        let rewards = rewards_since(rewards, now_ms.saturating_sub(REWARDS_WINDOW_MS)).unwrap_or(Dec::ZERO);
        Staking { summary, delegations, pending, rewards }
    }

    /// Everything staked: delegated, the staking balance, and what is on its
    /// way out, which is the account's until it lands in spot.
    #[must_use]
    pub fn total(&self) -> Option<Dec> {
        let summary = &self.summary;
        summary.delegated.checked_add(summary.undelegated)?.checked_add(summary.total_pending_withdrawal)
    }

    /// The delegation to `validator`, if any.
    #[must_use]
    pub fn delegation(&self, validator: Address) -> Option<&Delegation> {
        self.delegations.iter().find(|delegation| delegation.validator == validator)
    }
}

/// The four reads one after another, made into a [`Staking`] at `now_ms`.
pub async fn staking(
    transport: &impl Transport,
    network: Network,
    user: Address,
    now_ms: u64,
) -> Result<Staking, ApiError> {
    let summary = delegator_summary(transport, user).await?;
    let delegations = delegations(transport, user).await?;
    let rewards = delegator_rewards(transport, user).await?;
    let history = delegator_history(transport, user).await?;
    Ok(Staking::new(network, summary, delegations, &rewards, &history, now_ms))
}

#[cfg(test)]
#[path = "tests/staking.rs"]
mod tests;
