//! Hyperliquid SDK for Rust.
//!
//! - [`client`]: the [`Client`], `/info` and `/exchange` over HTTP, rate limits, HyperEVM, and
//!   the native and browser wires and websockets.
//! - [`markets`]: what is listed and what it trades at.
//! - [`account`]: what an address holds and who may sign for it.
//! - [`trade`]: orders on every market.
//! - [`transfer`]: moving money.
//! - [`earn`]: staking, vaults and lending.
//! - [`deploy`]: spot tokens, HIP-3 dexes and HIP-4 outcome markets.
//! - [`live`]: websocket subscriptions and kept live state.
//! - [`utils`]: decimals, rounding, asset ids, units and signing primitives.
//!
//! Money is exact decimals, never floats. Features: `sign` and `native` (default), `web`.

pub mod account;
pub mod client;
pub mod deploy;
pub mod earn;
pub mod live;
pub mod markets;
pub mod trade;
pub mod transfer;
pub mod utils;

#[cfg(fuzzing)]
#[doc(hidden)]
pub mod fuzz;

#[cfg(test)]
pub(crate) mod test_support;

pub use client::{Client, Network, SendError};
pub use utils::{Address, Dec};
