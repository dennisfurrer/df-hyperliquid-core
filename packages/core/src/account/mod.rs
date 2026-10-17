//! What an address holds and who may sign for it.
//!
//! - [`holdings`]: perp and spot balances, positions, what an order can draw on, worth, outcome sides.
//! - [`history`]: orders, fills and fees, the ledger, the portfolio.
//! - [`keys`]: signatures, which key may sign what, agents, builder approvals.
//! - [`settings`]: the account mode, subaccounts, referrals, rate limits.

pub mod history;
pub mod holdings;
pub mod keys;
pub mod settings;

pub use crate::utils::sign::{L1Action, UserAction};
pub use history::{
    DailyVolume, FeeSchedule, FeeTiers, FillLiquidation, FrontendOrder, History, MakerTier, OrderKind, OrderLookup,
    OrderRecord, OrderState, Portfolio, Span, StakingDiscount, TwapSliceFill, UserFees, VolumeTier, portfolio,
    portfolio_request, summed,
};
pub use holdings::{
    ActiveAssetData, AssetPosition, Balance, Beside, ClearinghouseState, Collateral, CumFunding, DexState, Direction,
    Keeping, Leverage, MarginSummary, Pays, Position, Prices, Risk, Source, SpotBalance, SpotState, Standing, Surface,
    Usable, Worth, active_asset_data, active_asset_data_request, clearinghouse_state, clearinghouse_state_request,
    combined, payer, spot_state, spot_state_request, standing,
};
pub use keys::{
    ActionKind, Agent, AgentName, ApproveAgent, ApproveBuilderFee, BUILDER_MINIMUM, BadAgentName, BadExpiry, Key,
    MAX_AGENT_VALIDITY_MS, NotReady, Readiness, Refused, Reused, Role, SignError, Signature, Signer, Signers, Why,
    builder_readiness, extra_agents, extra_agents_request, fresh_agent, max_builder_fee, max_builder_fee_request,
    may_sign, readiness, role, role_request, split_valid_until,
};
#[cfg(feature = "sign")]
pub use keys::{LocalKey, recover, verify};
pub use settings::{
    Abstraction, AbstractionMode, AccountRef, AgentSetAbstraction, BadCode, CreateSubAccount, LONGEST_CODE, RateLimit,
    Referral, ReferralState, ReferredBy, ReferrerData, ReferrerState, SetReferrer, SubAccount, SubAccountSpotTransfer,
    SubAccountTransfer, SubRefusal, TokenRewards, UserSetAbstraction, abstraction, abstraction_request, rate_limit,
    rate_limit_request, referral, referral_request, sub_accounts, sub_accounts_request,
};

#[cfg(test)]
#[path = "tests/vectors.rs"]
mod vectors;

#[cfg(test)]
#[path = "tests/reads.rs"]
mod reads;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
