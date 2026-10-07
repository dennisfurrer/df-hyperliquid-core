//! Hyperliquid accounts: clearinghouse and spot state, account modes (abstraction), agent wallets, builder fee approval, subaccounts, moves between the account's own ledgers.
//!
//! `approveAgent`, `approveBuilderFee` and `userSetAbstraction` are
//! user-signed: only the account's own wallet signs them. [`agent`] says what
//! an agent key can and cannot do, that there is no revoke, and why an
//! address that was an agent before is never approved again. Unified back to
//! standard is refused ([`mode`]).
#![deny(clippy::float_arithmetic)]

pub mod agent;
pub mod builder;
pub mod dollars;
pub mod mode;
pub mod referral;
pub mod state;
pub mod sub_account;

pub use agent::{Agent, ApproveAgent, Role, extra_agents, extra_agents_request, role, role_request};
pub use builder::{ApproveBuilderFee, max_builder_fee, max_builder_fee_request};
pub use dollars::Dollars;
use hl_core::{Address, ApiError, Transport, info};
pub use hl_core::{L1Action, UserAction};
pub use mode::{AbstractionMode, AgentSetAbstraction, UserSetAbstraction};
pub use referral::{
    BadCode, LONGEST_CODE, Referral, ReferredBy, ReferrerData, ReferrerState, SetReferrer, referral, referral_request,
};
pub use state::{
    Abstraction, MarginSummary, PerpState, SpotBalance, SpotState, abstraction_request, perp_state_request,
    spot_state_request,
};
pub use sub_account::{
    CreateSubAccount, SubAccount, SubAccountSpotTransfer, SubAccountTransfer, sub_accounts, sub_accounts_request,
};

pub async fn spot_state(transport: &impl Transport, user: Address) -> Result<SpotState, ApiError> {
    info(transport, spot_state_request(user)).await
}

pub async fn perp_state(transport: &impl Transport, user: Address) -> Result<PerpState, ApiError> {
    info(transport, perp_state_request(user)).await
}

pub async fn abstraction(transport: &impl Transport, user: Address) -> Result<Abstraction, ApiError> {
    info(transport, abstraction_request(user)).await
}

/// The three reads behind [`Dollars`], one after another. Per hip4-tui's
/// table they weigh 20 (`userAbstraction`) and 2 each for the two states.
pub async fn dollars(transport: &impl Transport, user: Address) -> Result<Dollars, ApiError> {
    let mode = abstraction(transport, user).await?;
    let spot = spot_state(transport, user).await?;
    let perp = perp_state(transport, user).await?;
    Dollars::new(&mode, &spot, &perp).ok_or_else(|| ApiError::Unreadable {
        path: hl_core::client::INFO,
        detail: format!("balances for {user} too large to add"),
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests/vectors.rs"]
mod vectors;

#[cfg(test)]
#[path = "tests/reads.rs"]
mod reads;
