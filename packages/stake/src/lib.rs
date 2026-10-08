//! HYPE staking: moves between spot and staking, delegate, undelegate, validators, rewards.
//!
//! HYPE moves from spot into the staking balance ([`CDeposit`]), is delegated
//! to a validator ([`TokenDelegate`]), and comes back the same way. All three
//! are user-signed: the account's own wallet signs them and an agent key is
//! refused. Amounts are `wei`, HYPE in 1e8 units.
//!
//! A delegation is locked before it can be undelegated, and a withdrawal to
//! spot waits in a queue, at most five at once. Both differ by network: a day
//! and seven days on mainnet, two and five minutes on testnet
//! ([`delegation_lock_ms`], [`unstaking_ms`]).
//!
//! [`Ctx`] plans a move: the action, its nonce, the [`Key`] it needs, the
//! [`Effect`] a read back should show and when the HYPE is free again.
//! [`staking`] reads an account's whole stake: delegations, the withdrawals
//! still queued, and a month's rewards.
#![deny(clippy::float_arithmetic)]

pub mod actions;
pub mod plan;
pub mod reads;
pub mod staking;

pub use actions::{CDeposit, CWithdraw, HYPE_DECIMALS, TokenDelegate, hype_wei};
pub use hl_core::UserAction;
pub use plan::{Balance, Change, Ctx, Effect, Key, Plan, Refused, key};
pub use reads::{
    Delegation, DelegatorSummary, MAINNET_DELEGATION_LOCK_MS, MAINNET_UNSTAKING_MS, MAX_PENDING_WITHDRAWALS, Reward,
    StakeDelta, StakeEvent, TESTNET_DELEGATION_LOCK_MS, TESTNET_UNSTAKING_MS, ValidatorStats, ValidatorSummary,
    delegation_lock_ms, delegations, delegations_request, delegator_history, delegator_history_request,
    delegator_rewards, delegator_rewards_request, delegator_summary, delegator_summary_request, locked_until,
    unstaking_ms, validator_summaries, validator_summaries_request,
};
pub use staking::{PendingWithdrawal, REWARDS_WINDOW_MS, Staking, pending_withdrawals, rewards_since, staking};
