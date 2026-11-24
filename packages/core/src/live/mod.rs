//! The websocket without the socket: subscriptions, the frames sent and received, the
//! connection's state, what a feed hands back, and a store that keeps live state for an app.
//! Rows the socket sends in the shapes `/info` answers are the [`crate::account`] and
//! [`crate::markets`] types; only the socket's own frames are defined here.

pub mod account;
pub mod feed;
pub mod frame;
pub mod market;
pub mod request;
pub mod session;
pub mod store;
pub mod subscription;

pub use account::{
    AllDexsState, Cancel, DexState, Funding, HistoricalOrders, LeadingVault, Ledger, Liquidation, OpenOrders,
    OrderUpdate, PerpDexState, SpotState, TwapEvent, TwapHistory, TwapSliceFills, TwapState, TwapStates, TwapStatus,
    UserEvent, UserFills, UserFundings, UserState, WebData3,
};
pub use feed::{Event, FeedError};
pub use frame::{Ack, Answer, BadFrame, Frame, Method, Refusal, Reply, decode};
pub use market::{BadDiff, Bbo, L2, L2Diff, L2Level, Mids};
pub use request::{PING, Post, post, subscribe, unsubscribe};
pub use session::{
    Limits, MAX_CONNECTIONS, MAX_INFLIGHT_POSTS, MAX_MESSAGES_PER_MINUTE, MAX_NEW_CONNECTIONS_PER_MINUTE,
    MAX_SUBSCRIPTIONS, MAX_USERS, Refused, Retry, Session, Tick, Timing,
};
pub use subscription::{BadSubscription, Subscription};

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
