//! The order rows the venue reads back, on every market kind, the same shapes whether `/info`
//! answers them or the socket sends them: a resting order, one with its trigger and kind, and an
//! order with where it stood.

use std::{borrow::Cow, fmt};

use serde::{Deserialize, Deserializer};

use crate::markets::Side;
use crate::trade::orders::{Cloid, Tif, Tpsl};
use crate::utils::Dec;

/// One `openOrders` row, and the order an `orderUpdates` frame carries. `reduceOnly` comes only
/// when set.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    /// `BTC`, `xyz:XYZ100` on a HIP-3 dex, `@107` or `PURR/USDC` for a pair, `#14730`.
    pub coin: String,
    /// The side it rests on.
    pub side: Side,
    /// The limit price (`limitPx`).
    pub limit_px: Dec,
    /// What is left; `0` on a position's take profit or stop loss.
    pub sz: Dec,
    /// The venue's order id (`oid`).
    pub oid: u64,
    /// When it was placed, milliseconds.
    pub timestamp: u64,
    /// The size when placed (`origSz`), in the coin's units.
    pub orig_sz: Dec,
    /// Whether it may only reduce a position (`reduceOnly`); `false` when the venue leaves it out.
    #[serde(default)]
    pub reduce_only: bool,
    /// The client's order id; `None` when the order was sent without one.
    #[serde(default)]
    pub cloid: Option<Cloid>,
}

/// What kind of order a `frontendOpenOrders` row is, in the venue's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderKind {
    /// A limit order (`Limit`).
    Limit,
    /// A market order (`Market`).
    Market,
    /// A stop loss that goes at market once it fires (`Stop Market`).
    StopMarket,
    /// A stop loss that rests as a limit order once it fires (`Stop Limit`).
    StopLimit,
    /// A take profit that goes at market once it fires (`Take Profit Market`).
    TakeProfitMarket,
    /// A take profit that rests as a limit order once it fires (`Take Profit Limit`).
    TakeProfitLimit,
    /// A kind this build does not know, kept as written.
    Other(String),
}

impl OrderKind {
    /// From the venue's words: `Stop Market`. A kind this build does not know is kept as
    /// written.
    #[must_use]
    pub fn parse(word: &str) -> OrderKind {
        match word {
            "Limit" => OrderKind::Limit,
            "Market" => OrderKind::Market,
            "Stop Market" => OrderKind::StopMarket,
            "Stop Limit" => OrderKind::StopLimit,
            "Take Profit Market" => OrderKind::TakeProfitMarket,
            "Take Profit Limit" => OrderKind::TakeProfitLimit,
            other => OrderKind::Other(other.to_owned()),
        }
    }

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
        Ok(OrderKind::parse(&<Cow<'de, str>>::deserialize(deserializer)?))
    }
}

/// One `frontendOpenOrders` row, the order inside `orderStatus` and `historicalOrders`, and what
/// the socket's `openOrders` and `userHistoricalOrders` send: an `openOrders` row with its
/// trigger and what kind of order it is.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendOrder {
    /// `BTC`, `xyz:XYZ100` on a HIP-3 dex, `@107` or `PURR/USDC` for a pair, `#14730`.
    pub coin: String,
    /// The side it rests on.
    pub side: Side,
    /// The limit price (`limitPx`).
    pub limit_px: Dec,
    /// What is left; `0` on a position TP/SL, which closes the whole position.
    pub sz: Dec,
    /// The venue's order id (`oid`).
    pub oid: u64,
    /// Milliseconds.
    pub timestamp: u64,
    /// `N/A`, `Price above 6.04`, or `Triggered` once a trigger has fired.
    pub trigger_condition: String,
    /// Whether it is a trigger order: a take profit or a stop loss (`isTrigger`).
    pub is_trigger: bool,
    /// `0.0` on an order that is not a trigger.
    pub trigger_px: Dec,
    /// The take profit and stop loss of an entry not yet filled.
    #[serde(default)]
    pub children: Vec<FrontendOrder>,
    /// Whether it is a take profit or stop loss on the whole position (`isPositionTpsl`).
    pub is_position_tpsl: bool,
    /// Whether it may only reduce a position (`reduceOnly`).
    pub reduce_only: bool,
    /// What kind of order it is (`orderType`).
    pub order_type: OrderKind,
    /// The size when placed (`origSz`), in the coin's units.
    pub orig_sz: Dec,
    /// As the venue writes it: `Gtc`, `Alo`, `Ioc`, `FrontendMarket`, `LiquidationMarket`;
    /// `null` on a trigger still waiting.
    #[serde(default)]
    pub tif: Option<String>,
    /// The client's order id; `None` when the order was sent without one.
    #[serde(default)]
    pub cloid: Option<Cloid>,
}

impl FrontendOrder {
    /// Where it fires, while it is a trigger waiting to.
    #[must_use]
    pub fn trigger(&self) -> Option<Dec> {
        (self.is_trigger && self.trigger_px.is_positive()).then_some(self.trigger_px)
    }

    /// Whether it is a take profit or a stop loss, by its kind; `None` for a plain order.
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

/// Where an order stands, by the kind of end the venue's word names. The word itself is kept on
/// a cancel or a rejection: `marginCanceled`, `scheduledCancel` (the dead man's switch),
/// `minTradeNtlRejected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderState {
    /// Resting on the book, or a trigger waiting to fire (`open`).
    Open,
    /// Filled in full (`filled`).
    Filled,
    /// A trigger that has fired (`triggered`).
    Triggered,
    /// Canceled: `canceled`, `scheduledCancel` or any word ending in `Canceled`, kept as written.
    Canceled(String),
    /// Rejected: `rejected` or any word ending in `Rejected`, kept as written.
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

/// An order and where it stood at `status_timestamp`: an `orderStatus` answer's order, each
/// `historicalOrders` row, and each order `userHistoricalOrders` sends.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    /// The order as placed.
    pub order: FrontendOrder,
    /// Where it stood (`status`).
    pub status: OrderState,
    /// Milliseconds.
    pub status_timestamp: u64,
}
