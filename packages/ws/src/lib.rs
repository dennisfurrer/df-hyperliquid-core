//! Hyperliquid's public websocket ([`hl_core::Network::ws_url`]), sans-IO: no socket, no async
//! runtime, no clock and no randomness inside. A transport opens the socket, hands in what
//! arrives with the time it arrived, and sends what comes back, natively, in the browser or in
//! a Worker.
//!
//! - [`Subscription`]: every feed the docs list, as the object the venue takes.
//! - [`request`]: the frames sent: subscribe, unsubscribe, ping, and posts carrying an info
//!   read or a signed action, by id.
//! - [`decode`]: a received frame read into a [`Frame`] per channel, prices and sizes as exact
//!   [`hl_core::Dec`]. An unknown channel is kept as written; a frame that does not read is an
//!   error, never a panic.
//! - [`Session`]: one connection's state: what it subscribes to, resubscribing on every open,
//!   the ping, the silence that means a dead socket, the reconnect delay, and the venue's
//!   per-IP limits, refused before anything is sent.
//!
//! The network is always named; nothing here assumes mainnet.
#![deny(clippy::float_arithmetic)]

pub mod account;
pub mod frame;
pub mod market;
pub mod request;
pub mod session;
pub mod subscription;

pub use account::{
    AllDexsState, AssetData, AssetPosition, Balance, Cancel, Clearinghouse, CumFunding, DexState, Fill,
    FillLiquidation, Funding, HistoricalOrder, HistoricalOrders, LeadingVault, Ledger, LedgerUpdate, Leverage,
    Liquidation, Margin, MarginSummary, OpenOrder, OpenOrders, Order, OrderUpdate, PerpDexState, Position, SpotState,
    TwapEvent, TwapHistory, TwapSliceFill, TwapSliceFills, TwapState, TwapStates, TwapStatus, UserEvent, UserFills,
    UserFundings, UserState, WebData3,
};
pub use frame::{Ack, Answer, BadFrame, Frame, Method, Refusal, Reply, decode};
pub use market::{Bbo, Book, Candle, Level, Mids, PerpCtx, Side, SpotCtx, Trade};
pub use request::{PING, Post, post, subscribe, unsubscribe};
pub use session::{
    Limits, MAX_CONNECTIONS, MAX_INFLIGHT_POSTS, MAX_MESSAGES_PER_MINUTE, MAX_NEW_CONNECTIONS_PER_MINUTE,
    MAX_SUBSCRIPTIONS, MAX_USERS, Refused, Retry, Session, Tick, Timing,
};
pub use subscription::{BadSubscription, Interval, Subscription};

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
