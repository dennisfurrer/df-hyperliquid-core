//! HYPE staking: moves between spot and staking, delegate, undelegate, validators, rewards.
//!
//! HYPE moves from spot into the staking balance ([`CDeposit`]), is delegated
//! to a validator ([`TokenDelegate`]), and comes back the same way. All three
//! are user-signed: the account's own wallet signs them and an agent key is
//! refused. Amounts are `wei`, HYPE in 1e8 units.
//!
//! A delegation is locked for [`DELEGATION_LOCK_MS`] before it can be
//! undelegated, and a withdrawal to spot waits [`UNSTAKING_MS`] in a queue.
#![deny(clippy::float_arithmetic)]

pub mod actions;
pub mod reads;

pub use actions::{CDeposit, CWithdraw, HYPE_DECIMALS, TokenDelegate, hype_wei};
pub use hl_core::UserAction;
pub use reads::{
    DELEGATION_LOCK_MS, Delegation, DelegatorSummary, Reward, StakeDelta, StakeEvent, UNSTAKING_MS, ValidatorStats,
    ValidatorSummary, delegations, delegations_request, delegator_history, delegator_history_request,
    delegator_rewards, delegator_rewards_request, delegator_summary, delegator_summary_request, validator_summaries,
    validator_summaries_request,
};
