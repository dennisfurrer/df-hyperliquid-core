//! Against mainnet, read only, run by hand with `just live`: every `/info` read the crate exposes,
//! one subscription per websocket channel, and HyperEVM's reads. Nothing is ever sent to
//! `/exchange`.
//!
//! Each read must parse into its type, and its answer must have the shape of the recording the
//! unit and mocked suites read ([`tap::same_shape`]), so a field the venue has added shows up
//! here before it reaches a user.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

mod shape;
mod tap;

mod account;
mod client;
mod deploy;
mod earn;
mod markets;
mod socket;
mod transfer;
