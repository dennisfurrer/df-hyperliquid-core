//! How an account is set up: its mode and changing it, its subaccounts, its referrer, its own code
//! and its referral rewards, and what it may still send and buying more.

pub mod limits;
pub mod mode;
pub mod referral;
pub mod subaccounts;

pub use limits::{
    BadWeight, INITIAL_REQUESTS, MOST_OPEN_ORDERS, MOST_RESERVED_WEIGHT, OPEN_ORDERS, RESERVED_REQUEST_COST, RateLimit,
    ReserveRequestWeight, THROTTLED_EVERY_MS, VOLUME_PER_OPEN_ORDER, address_requests, open_order_cap, rate_limit,
    rate_limit_request, surplus_cost, volume_for,
};
pub use mode::{
    Abstraction, AbstractionMode, AgentSetAbstraction, UserSetAbstraction, abstraction, abstraction_request,
};
pub use referral::{
    BadCode, ClaimRewards, LONGEST_CODE, Referral, ReferralState, ReferredBy, ReferrerData, ReferrerState, SetReferrer,
    TokenRewards, referral, referral_request,
};
pub use subaccounts::{
    AccountRef, CreateSubAccount, SubAccount, SubAccountSpotTransfer, SubAccountTransfer, SubRefusal, sub_accounts,
    sub_accounts_request,
};
