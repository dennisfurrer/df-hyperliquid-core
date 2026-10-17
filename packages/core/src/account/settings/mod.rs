//! How an account is set up: its mode and changing it, its subaccounts, its referrer and its own
//! code, and what it may still send.

pub mod limits;
pub mod mode;
pub mod referral;
pub mod subaccounts;

pub use limits::{
    INITIAL_REQUESTS, MOST_OPEN_ORDERS, OPEN_ORDERS, RateLimit, THROTTLED_EVERY_MS, VOLUME_PER_OPEN_ORDER,
    address_requests, open_order_cap, rate_limit, rate_limit_request, surplus_cost, volume_for,
};
pub use mode::{
    Abstraction, AbstractionMode, AgentSetAbstraction, UserSetAbstraction, abstraction, abstraction_request,
};
pub use referral::{
    BadCode, LONGEST_CODE, Referral, ReferralState, ReferredBy, ReferrerData, ReferrerState, SetReferrer, TokenRewards,
    referral, referral_request,
};
pub use subaccounts::{
    AccountRef, CreateSubAccount, SubAccount, SubAccountSpotTransfer, SubAccountTransfer, SubRefusal, sub_accounts,
    sub_accounts_request,
};
