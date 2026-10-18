//! Whole journeys through the public [`Client`](df_hyperliquid_core::Client) against a fake
//! venue that answers from recordings and checks every signed action the way the venue does
//! ([`support::venue::Venue`]), and the websocket feed against a local socket replaying recorded
//! frames. Each test is one thing a user does, end to end; the units it passes through have their
//! own tests next to their code.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

mod account;
mod client;
mod deploy;
mod earn;
mod markets;
mod socket;
mod trade;
mod transfer;
