//! An account's orders as the venue reads them back, on every market kind:
//! `openOrders`, `frontendOpenOrders`, `orderStatus` and `historicalOrders`.
//! Figures are exact decimals; sides are `B` (a buy) and `A` (a sell).

use crate::client::request::{body, dex, number, text, user};
use crate::client::{ApiError, Transport, info};
use crate::trade::manage::OrderRef;
use crate::utils::Address;
use serde::{Deserialize, Deserializer, de};

/// The order rows `/info` answers and the socket sends.
pub use crate::account::order::{FrontendOrder, OpenOrder, OrderKind, OrderRecord, OrderState};

/// `orderStatus`: the order, or `unknownOid` for one the venue never took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderLookup {
    Found(Box<OrderRecord>),
    Unknown,
}

impl<'de> Deserialize<'de> for OrderLookup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            status: String,
            #[serde(default)]
            order: Option<OrderRecord>,
        }
        let raw = Raw::deserialize(deserializer)?;
        match (raw.status.as_str(), raw.order) {
            ("order", Some(order)) => Ok(OrderLookup::Found(Box::new(order))),
            ("unknownOid", _) => Ok(OrderLookup::Unknown),
            (other, _) => Err(de::Error::custom(format!("orderStatus answered `{other}`"))),
        }
    }
}

/// `{"type":"openOrders","user":..,"dex"?}`. Spot orders come with the main
/// dex only.
#[must_use]
pub fn open_orders_request(account: Address, on: Option<&str>) -> String {
    body("openOrders", [user(account)].into_iter().chain(dex(on)))
}

/// `{"type":"frontendOpenOrders","user":..,"dex"?}`.
#[must_use]
pub fn frontend_open_orders_request(account: Address, on: Option<&str>) -> String {
    body("frontendOpenOrders", [user(account)].into_iter().chain(dex(on)))
}

/// `{"type":"orderStatus","user":..,"oid":..}`: the venue's id as a number,
/// or the client's as its hex.
#[must_use]
pub fn order_status_request(account: Address, target: OrderRef) -> String {
    let oid = match target {
        OrderRef::Oid(oid) => number(oid),
        OrderRef::Cloid(cloid) => text(&cloid.to_hex()),
    };
    body("orderStatus", [user(account), ("oid", oid)])
}

/// `{"type":"historicalOrders","user":..}`: at most the 2000 most recent.
#[must_use]
pub fn historical_orders_request(account: Address) -> String {
    body("historicalOrders", [user(account)])
}

/// The account's resting orders on one dex: the main dex when `on` is `None`.
pub async fn open_orders(
    transport: &impl Transport,
    account: Address,
    on: Option<&str>,
) -> Result<Vec<OpenOrder>, ApiError> {
    info(transport, open_orders_request(account, on)).await
}

/// The account's resting orders, with what kind each is.
pub async fn frontend_open_orders(
    transport: &impl Transport,
    account: Address,
    on: Option<&str>,
) -> Result<Vec<FrontendOrder>, ApiError> {
    info(transport, frontend_open_orders_request(account, on)).await
}

/// One order of the account, by either id.
pub async fn order_status(
    transport: &impl Transport,
    account: Address,
    target: OrderRef,
) -> Result<OrderLookup, ApiError> {
    info(transport, order_status_request(account, target)).await
}

/// The account's order history, newest first.
pub async fn historical_orders(transport: &impl Transport, account: Address) -> Result<Vec<OrderRecord>, ApiError> {
    info(transport, historical_orders_request(account)).await
}

#[cfg(test)]
#[path = "tests/orders.rs"]
mod tests;
