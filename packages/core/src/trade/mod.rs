//! Orders on every market: place, cancel, modify, TWAP, TP/SL, leverage and margin, builder fees, and the ticket that turns an intent into the exact action.
#![deny(clippy::float_arithmetic)]

pub mod action;
pub mod answer;
pub mod builder;
pub mod builder_fee;
pub mod cancel;
pub mod dexes;
pub mod error;
pub mod leverage;
pub mod margin;
pub mod modify;
pub mod order;
pub mod outcomes;
pub mod perps;
pub mod schedule;
pub mod ticket;
pub mod twap;

pub use crate::trade::builder_fee::BuilderFee;
pub use crate::utils::sign::L1Action;
pub use crate::utils::{Asset, AssetKind};
pub use action::TradeAction;
pub use answer::{CancelAnswer, OrderAnswer, TwapAnswer};
pub use builder::{Approval, ApprovalRefused, Approved, BuilderSetting, FeeRule, SignedBy};
pub use builder_fee::{OUTCOME_BUILDER_CAP, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
pub use cancel::{Cancel, CancelByCloid};
pub use error::TradeError;
pub use leverage::{MarginMode, UpdateIsolatedMargin, UpdateLeverage};
pub use margin::{FLAT_TABLES_BELOW, LeverageError, MarginTable, MarginTier, checked_leverage};
pub use modify::{BatchModify, Modify, OrderRef};
pub use order::{Builder, Cloid, Grouping, Order, OrderType, OverCap, PlaceOrder, Side, Tif, Tpsl};
pub use perps::{add_margin, remove_margin};
pub use schedule::{Noop, ScheduleCancel, TooSoon};
pub use ticket::{
    Amount, Change, Close, Covers, Exit, ExitPrice, Market, OrderIntent, Price, Refusal, Scale, TriggerIntent,
    TwapIntent,
};
pub use twap::{Twap, TwapCancel};

#[cfg(test)]
#[path = "tests/support.rs"]
pub(crate) mod tests_support;
