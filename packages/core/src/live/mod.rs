//! The websocket without the socket: subscriptions, frames, the connection's state, and a store that keeps live state for an app.
#![deny(clippy::float_arithmetic)]

pub mod account;
pub mod frame;
pub mod market;
pub mod request;
pub mod session;
pub mod store;
pub mod subscription;

pub use account::{
    AllDexsState, AssetData, AssetPosition, Balance, Cancel, Clearinghouse, CumFunding, Delta, DexState, Fill,
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
