//! What an address holds and who may sign for it: balances, positions, orders, fills, fees, modes, subaccounts, API wallets and builder approvals.
#![deny(clippy::float_arithmetic)]

pub mod active;
pub mod agent;
pub mod builder;
pub mod clearinghouse;
pub mod dollars;
pub mod fees;
pub mod fill;
pub mod fills;
pub mod ledger;
pub mod limits;
pub mod mode;
pub mod onboarding;
pub mod order;
pub mod orders;
pub mod outcomes;
pub mod portfolio;
pub mod positions;
pub mod readiness;
pub mod referral;
pub mod signer;
pub mod signers;
pub mod state;
pub mod sub_account;
pub mod usable;
pub mod worth;

use crate::client::{ApiError, Transport, info};
use crate::utils::Address;

pub use crate::utils::sign::{L1Action, UserAction};
pub use active::{ActiveAssetData, active_asset_data, active_asset_data_request};
pub use agent::{
    Agent, AgentName, ApproveAgent, BadAgentName, BadExpiry, MAX_AGENT_VALIDITY_MS, Reused, Role, extra_agents,
    extra_agents_request, fresh_agent, role, role_request, split_valid_until,
};
pub use builder::{ApproveBuilderFee, max_builder_fee, max_builder_fee_request};
pub use dollars::Dollars;
pub use fees::{Fees, fees, fees_request};
pub use fills::{
    DailyVolume, FeeSchedule, FeeTiers, FillLiquidation, MakerTier, StakingDiscount, TwapSliceFill, UserFees,
    VolumeTier,
};
pub use limits::{Budget, RateLimit, Sample, rate_limit, rate_limit_request};
pub use mode::{AbstractionMode, AgentSetAbstraction, UserSetAbstraction};
pub use onboarding::{Blocked, MIN_DEPOSIT, Seen, Status, Step, StepKind, Wants, deposited, left, steps};
pub use orders::{FrontendOrder, OrderKind, OrderLookup, OrderRecord, OrderState};
pub use outcomes::{Check, Direction, RawFill, RawOrder, SpotBalances, SpotRow, Summary, corroborate, summarize};
pub use portfolio::{History, Portfolio, Span, portfolio, portfolio_request, summed};
pub use positions::{
    AssetPosition, ClearinghouseState, CumFunding, Leverage, clearinghouse_state, clearinghouse_state_request,
};
pub use readiness::{
    Approved, BUILDER_MINIMUM, Clearance, NotReady, Readiness, builder_readiness, clearance, readiness,
};
pub use referral::{
    BadCode, LONGEST_CODE, Referral, ReferralState, ReferredBy, ReferrerData, ReferrerState, SetReferrer, TokenRewards,
    referral, referral_request,
};
#[cfg(feature = "sign")]
pub use signer::{LocalKey, recover, verify};
pub use signer::{SignError, Signature, Signer};
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

/// The three reads behind [`Dollars`], one after another. Weighs 24: 20 for `userAbstraction`
/// and 2 for each state.
pub async fn dollars(transport: &impl Transport, user: Address) -> Result<Dollars, ApiError> {
    let mode = abstraction(transport, user).await?;
    let spot = spot_state(transport, user).await?;
    let perp = perp_state(transport, user).await?;
    Dollars::new(&mode, &spot, &perp).ok_or_else(|| ApiError::Unreadable {
        path: crate::client::transport::INFO,
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
