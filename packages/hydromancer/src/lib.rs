//! Hydromancer, a third-party data provider for Hyperliquid: what the venue's own API does not
//! serve. Above all the L4 book, every resting order with its owner, as a snapshot
//! ([`snapshot`]) and a per-block stream ([`socket`]) that an [`Assembler`] keeps as one
//! [`Book`] a coin; and the outcome builder dashboard ([`dashboard`]), price history
//! ([`prices`]) and a builder's live fills. [`days`] reads Hyperliquid's own daily record of a
//! builder's fills, the figures the dashboard is checked against.
//!
//! - **Sans-IO**: requests go out through [`hl_core::Http`]; no socket, runtime or clock lives
//!   here. A socket's owner sends the frames built here and hands each one it receives to
//!   [`socket::read`].
//! - **Mainnet only**: Hydromancer's figures are mainnet's, so [`Hydromancer::mainnet`] is the
//!   one constructor and no testnet host is named anywhere.
//! - **The key is the caller's** ([`Key`]): held only by the client value, sent as a bearer
//!   header (the socket alone takes it in its URL, as a browser's socket carries no header),
//!   never printed, and cut out of any upstream text an error quotes.
//! - **Exact**: money and prices are [`hl_core::Dec`], read from the provider's decimal strings.
#![deny(clippy::float_arithmetic)]

pub mod assembler;
pub mod book;
pub mod client;
pub mod dashboard;
pub mod days;
pub mod prices;
pub mod snapshot;
pub mod socket;

pub use assembler::{Assembler, MAX_BUFFER, Step};
pub use book::{Book, Crossed, Level, levels};
pub use client::{API, BadCall, Call, DOCS, Error, Hydromancer, Key, NoKey, SOCKET};
pub use dashboard::{
    BuilderStats, BusiestDay, Category, CategoryStats, DailySeries, HoldTime, Hourly, HourlySeries, MAX_BUILDERS,
    MAX_DAYS, MAX_HOURS, Overall, Profitability, Span, VolumeDaily,
};
pub use days::{BUILDER_FILLS, BadDate, BuilderDay, Kind, Tally, builder_day, kind_of};
pub use prices::{History, PricePoint, PriceWindow, nearest, point_at_or_before};
pub use snapshot::{BadSnapshot, Market, Order, Side, Snapshot};
pub use socket::{BadFrame, Block, BuilderFill, Diff, Message, PING, PONG};
