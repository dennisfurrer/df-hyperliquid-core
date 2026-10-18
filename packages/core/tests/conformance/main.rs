//! Signing against other implementations: each action built through this crate's typed builders,
//! then its msgpack, hash and signature compared with what `@nktkas/hyperliquid`, the venue's
//! Python SDK and viem produce for the same input (`vectors/`).

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod base;
mod hip4;
mod lend;
mod stake;
mod trade;
mod transfer;
mod vault;
