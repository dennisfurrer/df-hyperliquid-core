//! Hyperliquid native lending and borrowing: reserves, rates, utilization, supply, withdraw, borrow, repay, health.
//!
//! [`BorrowLend`] is one L1 action for all four operations, naming the token
//! by its spot index; an agent key may sign it for its master. The reads say
//! what each reserve pays and charges and where an account stands.
#![deny(clippy::float_arithmetic)]

pub mod action;
pub mod state;

pub use action::{BadLend, BorrowLend, Operation};
pub use state::{
    Basis, Position, ReserveState, UserState, all_reserve_states, all_reserve_states_request, user_state,
    user_state_request,
};
