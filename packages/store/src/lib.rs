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
//!
//! A BTC page, from the socket's side:
//!
//! ```
//! use hl_core::{Dec, Network};
//! use hl_store::{Interest, Store};
//! use hl_ws::Session;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut store = Store::new(Network::Mainnet);
//! let mut session = Session::new(Network::Mainnet);
//! let book = store.register(Interest::Book { coin: "BTC".to_owned() })?;
//! store.sync(&mut session)?;
//! // The socket opens: the session subscribes to everything the store asked for.
//! let to_send = session.opened(0);
//! assert_eq!(to_send.len(), 1);
//!
//! let text = r#"{"channel":"l2Book","data":{"coin":"BTC","time":1,"levels":[[{"px":"100","sz":"1","n":1}],[{"px":"101","sz":"2","n":1}]]}}"#;
//! let frame = session.received(1_000, text)?;
//! store.apply(1_000, &frame);
//! assert_eq!(store.mid("BTC"), Some(Dec::parse("100.5")?));
//! assert_eq!(store.book("BTC").map(|book| book.age(4_000)), Some(3_000));
//!
//! // The page goes: its book goes, and the next sync unsubscribes it.
//! drop(book);
//! assert_eq!(store.sync(&mut session)?.frames.len(), 1);
//! assert!(store.book("BTC").is_none());
//! # Ok(())
//! # }
//! ```
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
