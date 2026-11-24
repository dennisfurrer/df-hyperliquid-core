//! An account's orders on the socket: the resting ones on one dex, each status change as it
//! happens, and the order history.

use crate::account::history::OpenOrder;
use crate::account::{FrontendOrder, OrderRecord};
use crate::utils::Address;
use serde::Deserialize;

/// One order's new status on `orderUpdates`: `open`, `filled`, `canceled`, `triggered`,
/// `rejected`, `marginCanceled`, `badAloPxRejected` and the rest the info docs list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdate {
    /// The order as it stands after the change.
    pub order: OpenOrder,
    /// The new status, as the venue writes it: `open`, `filled`, `canceled` and so on.
    pub status: String,
    /// Milliseconds.
    pub status_timestamp: u64,
}

/// `openOrders`: every resting order on one dex, with its trigger and type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OpenOrders {
    /// The dex, `""` for the first perp dex.
    pub dex: String,
    /// The account the orders belong to.
    pub user: Address,
    /// Every resting order on the dex.
    pub orders: Vec<FrontendOrder>,
}

/// `userHistoricalOrders`: orders and what became of them, the latest first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalOrders {
    /// The account the orders belong to.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past orders; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// `orderHistory`: the orders in this frame, each with its last status.
    #[serde(rename = "orderHistory")]
    pub orders: Vec<OrderRecord>,
}

#[cfg(test)]
#[path = "tests/orders.rs"]
mod tests;
