//! Live state kept from the websocket, sans-IO: markets, accounts, and the subscriptions that serve them.
//!
//! ```
//! use df_hyperliquid_core::utils::Dec;
//! use df_hyperliquid_core::client::Network;
//! use df_hyperliquid_core::live::store::{Interest, Store};
//! use df_hyperliquid_core::live::Session;
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
//! // Dropping the handle drops the book, and the next sync unsubscribes it.
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
pub mod state;

pub use account::{Account, KEEP};
pub use interest::{Changes, Handle, Interest};
pub use market::{BARS, Context, TAPE};
pub use stamp::Stamped;
pub use state::{Folded, OtherNetwork, Read, Store, Synced};

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
