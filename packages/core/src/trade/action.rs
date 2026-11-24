//! Every trading action as one value, so what a confirmation shows and what is signed are the
//! same value.

use crate::trade::manage::{BatchModify, Modify};
use crate::trade::manage::{Cancel, CancelByCloid};
use crate::trade::manage::{Noop, ScheduleCancel};
use crate::trade::margin::{UpdateIsolatedMargin, UpdateLeverage};
use crate::trade::orders::{PlaceOrder, TrailingStop};
use crate::trade::orders::{Twap, TwapCancel};
use crate::utils::sign::{L1Action, Map};

/// One trading action of any kind, signed as the action it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TradeAction {
    /// `order`: one or more orders.
    Place(PlaceOrder),
    /// `cancel`: orders by their oid.
    Cancel(Cancel),
    /// `cancelByCloid`: orders by their client order id.
    CancelByCloid(CancelByCloid),
    /// `modify`: one resting order replaced in place.
    Modify(Modify),
    /// `batchModify`: several resting orders replaced in place.
    BatchModify(BatchModify),
    /// `twapOrder`: a TWAP.
    Twap(Twap),
    /// `twapCancel`: a running TWAP stopped.
    TwapCancel(TwapCancel),
    /// `updateLeverage`: a perp's leverage and margin mode.
    Leverage(UpdateLeverage),
    /// `updateIsolatedMargin`: USDC moved into or out of an isolated position.
    Margin(UpdateIsolatedMargin),
    /// `scheduleCancel`: the dead man's switch armed or disarmed.
    ScheduleCancel(ScheduleCancel),
    /// `noop`: a nonce spent so an order signed with it never lands.
    Noop(Noop),
    /// `trailingStop`: a stop that trails the best price.
    TrailingStop(TrailingStop),
}

impl TradeAction {
    /// The action's `type`, which is also what an answer about it names.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            TradeAction::Place(_) => "order",
            TradeAction::Cancel(_) => "cancel",
            TradeAction::CancelByCloid(_) => "cancelByCloid",
            TradeAction::Modify(_) => "modify",
            TradeAction::BatchModify(_) => "batchModify",
            TradeAction::Twap(_) => "twapOrder",
            TradeAction::TwapCancel(_) => "twapCancel",
            TradeAction::Leverage(_) => "updateLeverage",
            TradeAction::Margin(_) => "updateIsolatedMargin",
            TradeAction::ScheduleCancel(_) => "scheduleCancel",
            TradeAction::Noop(_) => "noop",
            TradeAction::TrailingStop(_) => "trailingStop",
        }
    }

    /// How many statuses the venue answers it with, one an order, cancel or
    /// modify; `None` for an action answered by one status or none.
    #[must_use]
    pub fn items(&self) -> Option<usize> {
        match self {
            TradeAction::Place(place) => Some(place.orders().len()),
            TradeAction::Cancel(cancel) => Some(cancel.cancels().len()),
            TradeAction::CancelByCloid(cancel) => Some(cancel.cancels().len()),
            TradeAction::BatchModify(batch) => Some(batch.modifies().len()),
            _ => None,
        }
    }
}

impl L1Action for TradeAction {
    fn action(&self) -> Map {
        match self {
            TradeAction::Place(action) => action.action(),
            TradeAction::Cancel(action) => action.action(),
            TradeAction::CancelByCloid(action) => action.action(),
            TradeAction::Modify(action) => action.action(),
            TradeAction::BatchModify(action) => action.action(),
            TradeAction::Twap(action) => action.action(),
            TradeAction::TwapCancel(action) => action.action(),
            TradeAction::Leverage(action) => action.action(),
            TradeAction::Margin(action) => action.action(),
            TradeAction::ScheduleCancel(action) => action.action(),
            TradeAction::Noop(action) => action.action(),
            TradeAction::TrailingStop(action) => action.action(),
        }
    }
}

macro_rules! from {
    ($($action:ident => $variant:ident),* $(,)?) => {
        $(impl From<$action> for TradeAction {
            fn from(action: $action) -> TradeAction {
                TradeAction::$variant(action)
            }
        })*
    };
}

from! {
    PlaceOrder => Place,
    Cancel => Cancel,
    CancelByCloid => CancelByCloid,
    Modify => Modify,
    BatchModify => BatchModify,
    Twap => Twap,
    TwapCancel => TwapCancel,
    UpdateLeverage => Leverage,
    UpdateIsolatedMargin => Margin,
    ScheduleCancel => ScheduleCancel,
    Noop => Noop,
    TrailingStop => TrailingStop,
}

#[cfg(test)]
#[path = "tests/action.rs"]
mod tests;
