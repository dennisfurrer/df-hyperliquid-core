//! An account's orders as the venue reads them back, on every market kind:
//! `openOrders`, `frontendOpenOrders`, `orderStatus` and `historicalOrders`.
//! Figures are exact decimals; sides are `B` (a buy) and `A` (a sell).

use std::fmt;

use hl_core::{Address, ApiError, Dec, Transport, info};
use serde::{Deserialize, Deserializer, de};

use crate::{
    modify::OrderRef,
    order::{Cloid, Side, Tif, Tpsl},
    request::{body, dex, number, text, user},
};

/// One `openOrders` row. `reduceOnly` comes only when set.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    /// `BTC`, `xyz:XYZ100` on a HIP-3 dex, `@107` or `PURR/USDC` for a pair.
    pub coin: String,
    pub side: Side,
    pub limit_px: Dec,
    /// What is left; `0` on a position's take profit or stop loss.
    pub sz: Dec,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(default)]
    pub orig_sz: Option<Dec>,
    #[serde(default)]
    pub reduce_only: bool,
    #[serde(default)]
    pub cloid: Option<Cloid>,
}

/// What kind of order a `frontendOpenOrders` row is, in the venue's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderKind {
    Limit,
    Market,
    StopMarket,
    StopLimit,
    TakeProfitMarket,
    TakeProfitLimit,
    /// A kind this build does not know, kept as written.
    Other(String),
}

impl OrderKind {
    /// As the venue writes it: `Stop Market`.
    #[must_use]
    pub fn word(&self) -> &str {
        match self {
            OrderKind::Limit => "Limit",
            OrderKind::Market => "Market",
            OrderKind::StopMarket => "Stop Market",
            OrderKind::StopLimit => "Stop Limit",
            OrderKind::TakeProfitMarket => "Take Profit Market",
            OrderKind::TakeProfitLimit => "Take Profit Limit",
            OrderKind::Other(word) => word,
        }
    }

    /// Which trigger it is; `None` for a plain order.
    #[must_use]
    pub const fn tpsl(&self) -> Option<Tpsl> {
        match self {
            OrderKind::StopMarket | OrderKind::StopLimit => Some(Tpsl::StopLoss),
            OrderKind::TakeProfitMarket | OrderKind::TakeProfitLimit => Some(Tpsl::TakeProfit),
            _ => None,
        }
    }

    /// Whether it goes at market (once it fires, for a trigger).
    #[must_use]
    pub const fn fills_at_market(&self) -> bool {
        matches!(self, OrderKind::Market | OrderKind::StopMarket | OrderKind::TakeProfitMarket)
    }
}

impl<'de> Deserialize<'de> for OrderKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let word = String::deserialize(deserializer)?;
        Ok(match word.as_str() {
            "Limit" => OrderKind::Limit,
            "Market" => OrderKind::Market,
            "Stop Market" => OrderKind::StopMarket,
            "Stop Limit" => OrderKind::StopLimit,
            "Take Profit Market" => OrderKind::TakeProfitMarket,
            "Take Profit Limit" => OrderKind::TakeProfitLimit,
            _ => OrderKind::Other(word),
        })
    }
}

/// One `frontendOpenOrders` row, and the order inside `orderStatus` and
/// `historicalOrders`: an `openOrders` row with what kind of order it is.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendOrder {
    pub coin: String,
    pub side: Side,
    pub limit_px: Dec,
    pub sz: Dec,
    pub oid: u64,
    pub timestamp: u64,
    /// `N/A`, `Price above 6.04`, or `Triggered` once a trigger has fired.
    pub trigger_condition: String,
    pub is_trigger: bool,
    /// `0.0` on an order that is not a trigger.
    pub trigger_px: Dec,
    /// The take profit and stop loss of an entry not yet filled.
    #[serde(default)]
    pub children: Vec<FrontendOrder>,
    pub is_position_tpsl: bool,
    pub reduce_only: bool,
    pub order_type: OrderKind,
    pub orig_sz: Dec,
    /// As the venue writes it: `Gtc`, `Alo`, `Ioc`, `FrontendMarket`,
    /// `LiquidationMarket`; `null` on a trigger still waiting.
    #[serde(default)]
    pub tif: Option<String>,
    #[serde(default)]
    pub cloid: Option<Cloid>,
}

impl FrontendOrder {
    /// Where it fires, while it is a trigger waiting to.
    #[must_use]
    pub fn trigger(&self) -> Option<Dec> {
        (self.is_trigger && self.trigger_px.is_positive()).then_some(self.trigger_px)
    }

    #[must_use]
    pub const fn tpsl(&self) -> Option<Tpsl> {
        self.order_type.tpsl()
    }

    /// The time in force, when it is one an order may be sent with.
    #[must_use]
    pub fn time_in_force(&self) -> Option<Tif> {
        match self.tif.as_deref()? {
            "Gtc" => Some(Tif::Gtc),
            "Alo" => Some(Tif::Alo),
            "Ioc" => Some(Tif::Ioc),
            "FrontendMarket" => Some(Tif::FrontendMarket),
            _ => None,
        }
    }
}

/// Where an order stands, by the kind of end the venue's word names. The
/// word itself is kept on a cancel or a rejection: `marginCanceled`,
/// `scheduledCancel` (the dead man's switch), `minTradeNtlRejected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderState {
    Open,
    Filled,
    Triggered,
    Canceled(String),
    Rejected(String),
    /// A word this build does not know.
    Other(String),
}

impl OrderState {
    /// As the venue wrote it.
    #[must_use]
    pub fn word(&self) -> &str {
        match self {
            OrderState::Open => "open",
            OrderState::Filled => "filled",
            OrderState::Triggered => "triggered",
            OrderState::Canceled(word) | OrderState::Rejected(word) | OrderState::Other(word) => word,
        }
    }
}

impl<'de> Deserialize<'de> for OrderState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let word = String::deserialize(deserializer)?;
        Ok(match word.as_str() {
            "open" => OrderState::Open,
            "filled" => OrderState::Filled,
            "triggered" => OrderState::Triggered,
            "canceled" | "scheduledCancel" => OrderState::Canceled(word),
            canceled if canceled.ends_with("Canceled") => OrderState::Canceled(word),
            rejected if rejected == "rejected" || rejected.ends_with("Rejected") => OrderState::Rejected(word),
            _ => OrderState::Other(word),
        })
    }
}

impl fmt::Display for OrderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.word())
    }
}

/// An order and where it stood at `status_timestamp`: an `orderStatus`
/// answer's order, and each `historicalOrders` row.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub order: FrontendOrder,
    pub status: OrderState,
    pub status_timestamp: u64,
}

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
