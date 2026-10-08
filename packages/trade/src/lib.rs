//! Hyperliquid trading shared by every market kind: order, cancel, modify, scheduleCancel, noop, TWAP, leverage and margin; margin tables and the leverage a perp allows; tick and lot rounding; the ticket from intent to action (market orders are FrontendMarket, never Gtc); builder fees as a setting and their approval; what /exchange answers; order, fill and market reads.
//!
//! Every action here is an L1 action ([`L1Action`]): an approved agent key may
//! sign it for its master. A market order is [`Order::market`], which is always
//! `FrontendMarket`: a `Gtc` at the same price is a different order, one that
//! rests on the book when it does not fill.
//!
//! Prices and sizes go on the wire exactly as given. [`Grid`] puts them on an
//! asset's grid first, the way the venue's own clients do: a price never
//! rounds against the trader, a size rounds toward zero. [`ticket`] does it
//! all from what a trader means: the exact action, or why there is none.
//!
//! No builder is named here: [`BuilderSetting`] is the caller's, held to each
//! market kind's cap, and [`BuilderSetting::approval`] says whether an order
//! goes, needs `approveBuilderFee` first, or is refused.
//!
//! Reads go over any [`hl_core::Transport`]; [`answer`] reads what
//! `/exchange` says of each order, cancel or modify. Nothing here holds a
//! clock or does I/O of its own.
#![deny(clippy::float_arithmetic)]

pub mod action;
pub mod answer;
pub mod builder;
pub mod cancel;
pub mod error;
pub mod fills;
pub mod grid;
pub mod leverage;
pub mod margin;
pub mod market;
pub mod modify;
pub mod order;
pub mod orders;
mod request;
pub mod schedule;
pub mod ticket;
pub mod twap;

pub use action::TradeAction;
pub use answer::{CancelAnswer, OrderAnswer, TwapAnswer};
pub use builder::{Approval, ApprovalRefused, Approved, BuilderSetting, FeeRule, SignedBy};
pub use cancel::{Cancel, CancelByCloid};
pub use error::TradeError;
pub use fills::{
    DailyVolume, FeeSchedule, FeeTiers, Fill, MakerTier, StakingDiscount, TwapSliceFill, UserFees, VolumeTier,
};
pub use grid::{Grid, MAX_SLIPPAGE, MINIMUM_ORDER_VALUE, PERP_PRICE_PLACES, SIGNIFICANT_FIGURES, SPOT_PRICE_PLACES};
pub use hl_core::{Asset, AssetKind, BuilderFee, L1Action};
pub use leverage::{MarginMode, UpdateIsolatedMargin, UpdateLeverage};
pub use margin::{FLAT_TABLES_BELOW, LeverageError, MarginTable, MarginTier, checked_leverage};
pub use market::{Candle, Interval, L2Book, Level, MarketTrade, Mids, Precision, UnknownInterval};
pub use modify::{BatchModify, Modify, OrderRef};
pub use order::{Builder, Cloid, Grouping, Order, OrderType, OverCap, PlaceOrder, Side, Tif, Tpsl};
pub use orders::{FrontendOrder, OpenOrder, OrderKind, OrderLookup, OrderRecord, OrderState};
pub use schedule::{Noop, ScheduleCancel, TooSoon};
pub use ticket::{
    Amount, Change, Close, Covers, Exit, ExitPrice, Market, OrderIntent, Price, Refusal, Scale, TriggerIntent,
    TwapIntent,
};
pub use twap::{Twap, TwapCancel};

#[cfg(test)]
#[path = "tests/support.rs"]
mod tests_support;
