//! Hyperliquid accounts: clearinghouse and spot state, account modes
//! (abstraction) and changing them, agent wallets, builder fee approval,
//! subaccounts and referrals.
//!
//! - [`state`]: spot and each perp dex's clearinghouse, and the mode.
//! - [`usable`]: what an account can put on each kind of market, read the way
//!   its mode says, and which balance pays for an order.
//! - [`worth`]: what an account is worth, counted once, and what it holds
//!   beside its balances.
//! - [`onboarding`]: what is left before an account can trade, from live state.
//! - [`readiness`]: whether a builder can be paid, and what an order naming
//!   one needs first.
//! - [`limits`]: what an address may still send, and how fast that runs down.
//! - [`signers`]: who may sign what (hl-core's); [`agent`]: agents, which there is no
//!   revoking, so an agent address is never used twice.
//! - [`sub_account`]: subaccounts, and what a subaccount refuses.
//! - [`fees`], [`portfolio`], [`referral`]: the account's other reads.
//!
//! `approveAgent`, `approveBuilderFee` and `userSetAbstraction` are
//! user-signed: only the account's own wallet signs them. Unified back to
//! standard is refused ([`mode`]). Everything is a pure function over what
//! the caller read, or one read over a [`Transport`]: no I/O and no clock.
#![deny(clippy::float_arithmetic)]

pub mod agent;
pub mod builder;
pub mod dollars;
pub mod fees;
pub mod limits;
pub mod mode;
pub mod onboarding;
pub mod portfolio;
pub mod readiness;
pub mod referral;
pub mod state;
pub mod sub_account;
pub mod usable;
pub mod worth;

pub use agent::{
    Agent, AgentName, ApproveAgent, BadAgentName, BadExpiry, MAX_AGENT_VALIDITY_MS, Reused, Role, extra_agents,
    extra_agents_request, fresh_agent, role, role_request, split_valid_until,
};
pub use builder::{ApproveBuilderFee, max_builder_fee, max_builder_fee_request};
pub use dollars::Dollars;
pub use fees::{Fees, fees, fees_request};
/// Who may sign what lives in hl-core, where the money crates read it too.
pub use hl_core::signers;
use hl_core::{Address, ApiError, Transport, info};
pub use hl_core::{L1Action, UserAction};
pub use limits::{Budget, RateLimit, Sample, rate_limit, rate_limit_request};
pub use mode::{AbstractionMode, AgentSetAbstraction, UserSetAbstraction};
pub use onboarding::{Blocked, MIN_DEPOSIT, Seen, Status, Step, StepKind, Wants, deposited, left, steps};
pub use portfolio::{History, Portfolio, Span, portfolio, portfolio_request, summed};
pub use readiness::{
    Approved, BUILDER_MINIMUM, Clearance, NotReady, Readiness, builder_readiness, clearance, readiness,
};
pub use referral::{
    BadCode, LONGEST_CODE, Referral, ReferralState, ReferredBy, ReferrerData, ReferrerState, SetReferrer, referral,
    referral_request,
};
pub use signers::{ActionKind, Key, Refused, Signers, Why, may_sign};
pub use state::{
    Abstraction, MarginSummary, PerpState, Position, Risk, SpotBalance, SpotRisk, SpotState, abstraction_request,
    perp_state_on_request, perp_state_request, spot_state_request,
};
pub use sub_account::{
    AccountRef, CreateSubAccount, SubAccount, SubAccountSpotTransfer, SubAccountTransfer, SubRefusal, sub_accounts,
    sub_accounts_request,
};
pub use usable::{
    Balance, Collateral, DexState, Draws, Keeping, Move, Pays, Source, Standing, Standings, Surface, Usable, payer,
};
pub use worth::{Beside, Holding, Holdings, Prices, Worth, combined, holdings};

/// Weighs 2.
pub async fn spot_state(transport: &impl Transport, user: Address) -> Result<SpotState, ApiError> {
    info(transport, spot_state_request(user)).await
}

/// The main dex. Weighs 2.
pub async fn perp_state(transport: &impl Transport, user: Address) -> Result<PerpState, ApiError> {
    info(transport, perp_state_request(user)).await
}

/// One dex, `""` the main one. Weighs 2.
pub async fn perp_state_on(transport: &impl Transport, user: Address, dex: &str) -> Result<PerpState, ApiError> {
    info(transport, perp_state_on_request(user, dex)).await
}

/// Weighs 20.
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

/// One account as a [`Standing`]: its mode, spot, and each of `dexes` (`""`
/// the main one), one read after another at its own address (a
/// subaccount's for one: [`AccountRef::read_at`]). Weighs 22 and 2 a dex.
pub async fn standing(transport: &impl Transport, user: Address, dexes: &[&str]) -> Result<Standing, ApiError> {
    let mode = abstraction(transport, user).await?;
    let spot = spot_state(transport, user).await?;
    let mut read = Vec::with_capacity(dexes.len());
    for dex in dexes {
        read.push(DexState { dex: (*dex).to_owned(), state: perp_state_on(transport, user, dex).await? });
    }
    Ok(Standing { mode: Some(mode), dexes: read, spot: Some(spot), heard: None })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "tests/vectors.rs"]
mod vectors;

#[cfg(test)]
#[path = "tests/reads.rs"]
mod reads;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
