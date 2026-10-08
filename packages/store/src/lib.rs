//! The live state an app keeps, so a perps, spot or predictions page starts from its views and
//! not from a state layer of its own. Sans-IO: no socket, no runtime and no clock inside; time
//! is handed in as milliseconds. It builds for wasm32 as it does natively.
//!
//! - **Interests** ([`Interest`]): a page registers what it needs and holds a [`Handle`];
//!   dropping it ends the interest. Interests are counted, and [`Store::changes`] (or
//!   [`Store::sync`], into an [`hl_ws::Session`]) gives what to subscribe and what to
//!   unsubscribe once nothing needs it, within the venue's per-IP limits. Nothing polls on its
//!   own (atlas's hub rule).
//! - **Folding** ([`Store::apply`], [`Store::read`]): socket frames and the crates' typed
//!   `/info` answers update the state per market (book, prints, candles, mids, contexts) and per
//!   account and dex (clearinghouse, spot balances, mode, resting orders, fills, fundings,
//!   ledger). Snapshot-then-delta channels are told apart, a fill is never held twice, and each
//!   piece is [`Stamped`] with when it was last told and last changed, for staleness.
//! - **Views**: positions, resting orders, what an account can use on a market (through
//!   hl-account, the way its mode says), worth counted once, the touch, the mid and a market's
//!   context. Views only read.
//! - **Accounts**: up to the venue's ten users; a subaccount is read at its own address, with its
//!   own mode.
//! - **Network**: one store is one network, never mixed (hip4-tui's D13).
#![deny(clippy::float_arithmetic)]

pub mod account;
mod convert;
pub mod interest;
pub mod market;
pub mod stamp;
pub mod store;

pub use account::{Account, KEEP};
pub use interest::{Changes, Handle, Interest};
pub use market::{BARS, Context, TAPE};
pub use stamp::Stamped;
pub use store::{Folded, OtherNetwork, Read, Store, Synced};

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "tests/interest.rs"]
mod interest_tests;

#[cfg(test)]
#[path = "tests/market.rs"]
mod market_tests;

#[cfg(test)]
#[path = "tests/account.rs"]
mod account_tests;

#[cfg(test)]
#[path = "tests/views.rs"]
mod views_tests;
