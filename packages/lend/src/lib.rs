//! Hyperliquid native lending and borrowing: reserves, rates, utilization, supply, withdraw, borrow, repay, health.
//!
//! [`BorrowLend`] is one L1 action for all four operations, naming the token
//! by its spot index from the caller's `spotMeta` (HYPE is 150 on mainnet and
//! 1105 on testnet); an agent key may sign it for its master. The reads say
//! what each reserve pays and charges and where an account stands; [`math`]
//! works out utilization, the rates, what a holding backs and what can still
//! be borrowed; [`Ctx`] plans a move with the [`Key`] it needs and the
//! [`Effect`] a read back should show.
#![deny(clippy::float_arithmetic)]

pub mod action;
pub mod math;
pub mod plan;
pub mod state;

pub use action::{BadLend, BorrowLend, Operation};
pub use math::{PROTOCOL_SHARE, RATE_PLACES, USER_BORROW_CAP, borrow_rate, borrowable, supply_rate};
pub use plan::{Balance, Change, Ctx, Effect, Key, Plan, Refused, key};
pub use state::{
    Basis, Position, ReserveState, Risk, UserState, all_reserve_states, all_reserve_states_request, reserve,
    reserve_state, reserve_state_request, user_state, user_state_request,
};
