//! Lending and borrowing: reserves, rates, supply, withdraw, borrow, repay and health.
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
