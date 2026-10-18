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
    pub order: OpenOrder,
    pub status: String,
    /// Milliseconds.
    pub status_timestamp: u64,
}

/// `openOrders`: every resting order on one dex, with its trigger and type.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OpenOrders {
    pub dex: String,
    pub user: Address,
    pub orders: Vec<FrontendOrder>,
}

/// `userHistoricalOrders`: orders and what became of them, the latest first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalOrders {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "orderHistory")]
    pub orders: Vec<OrderRecord>,
}

#[cfg(test)]
#[path = "tests/orders.rs"]
mod tests;
