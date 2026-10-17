//! Orders on every market: place, cancel, modify, TWAP, TP/SL, leverage and margin, builder fees, and the ticket that turns an intent into the exact action.
#![deny(clippy::float_arithmetic)]

pub mod action;
pub mod answer;
pub mod builder;
pub mod error;
pub mod manage;
pub mod margin;
pub mod orders;
pub mod outcomes;
pub mod ticket;

pub use crate::utils::sign::L1Action;
pub use crate::utils::{Asset, AssetKind};
pub use action::TradeAction;
pub use answer::{CancelAnswer, OrderAnswer, TwapAnswer};
pub use builder::{
    Approval, ApprovalRefused, Approved, Builder, BuilderFee, BuilderSetting, FeeRule, OUTCOME_BUILDER_CAP, OverCap,
    PERP_BUILDER_CAP, SPOT_BUILDER_CAP, SignedBy,
};
pub use error::TradeError;
pub use manage::{BatchModify, Cancel, CancelByCloid, Change, Modify, Noop, OrderRef, ScheduleCancel, TooSoon};
pub use margin::{
    FLAT_TABLES_BELOW, LeverageError, MarginMode, MarginTable, MarginTier, UpdateIsolatedMargin, UpdateLeverage,
    add_margin, checked_leverage, remove_margin,
};
pub use orders::{
    Cloid, Close, Covers, Exit, ExitPrice, Grouping, Order, OrderIntent, OrderType, PlaceOrder, Price, Scale, Side,
    Tif, Tpsl, TriggerIntent, Twap, TwapCancel, TwapIntent,
};
pub use ticket::{Amount, Market, Refusal};

#[cfg(test)]
#[path = "tests/support.rs"]
pub(crate) mod tests_support;

#[cfg(test)]
#[path = "tests/ticket_support.rs"]
pub(crate) mod ticket_support;
