//! An account's orders as the venue reads them back, on every market kind, the same shapes
//! whether `/info` answers them or the socket sends them: `openOrders` rows and the order inside
//! `orderUpdates`; `frontendOpenOrders` rows, the order inside `orderStatus` and
//! `historicalOrders`, and the socket's `openOrders` and `userHistoricalOrders`. With them the
//! words an order is placed with: its time in force, its trigger, its client id.

use std::{borrow::Cow, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de};

use crate::{
    dec::Dec,
    hex::{self, BadHex},
    side::Side,
};

/// How long an order may rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tif {
    /// Add liquidity only: refused if it would cross.
    Alo,
    /// Immediate or cancel.
    Ioc,
    /// Rests until filled or cancelled. Never a market order.
    Gtc,
    /// The venue's market order: crosses up to its price and never rests. Only hl-trade's
    /// `Order::market` makes one.
    FrontendMarket,
}

impl Tif {
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Tif::Alo => "Alo",
            Tif::Ioc => "Ioc",
            Tif::Gtc => "Gtc",
            Tif::FrontendMarket => "FrontendMarket",
        }
    }
}

/// Which way a trigger fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tpsl {
    /// When the price moves the position's way.
    TakeProfit,
    /// When it moves against it.
    StopLoss,
}

impl Tpsl {
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Tpsl::TakeProfit => "tp",
            Tpsl::StopLoss => "sl",
        }
    }
}

/// A client order id: 128 bits, written as `0x` and 32 hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cloid([u8; 16]);

impl Cloid {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Cloid {
        Cloid(bytes)
    }

    #[must_use]
    pub const fn from_u128(value: u128) -> Cloid {
        Cloid(value.to_be_bytes())
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode_prefixed(&self.0)
    }
}

impl FromStr for Cloid {
    type Err = BadHex;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        hex::decode_array(text).map(Cloid)
    }
}

impl fmt::Display for Cloid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// From the hex the venue writes for an order's `cloid`.
impl<'de> Deserialize<'de> for Cloid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Cow<'de, str>>::deserialize(deserializer)?.parse().map_err(de::Error::custom)
    }
}

/// One `openOrders` row, and the order an `orderUpdates` frame carries. `reduceOnly` comes only
/// when set.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    /// `BTC`, `xyz:XYZ100` on a HIP-3 dex, `@107` or `PURR/USDC` for a pair, `#14730`.
    pub coin: String,
    /// The side it rests on.
    pub side: Side,
    pub limit_px: Dec,
    /// What is left; `0` on a position's take profit or stop loss.
    pub sz: Dec,
    pub oid: u64,
    /// When it was placed, milliseconds.
    pub timestamp: u64,
    pub orig_sz: Dec,
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

/// One `frontendOpenOrders` row, the order inside `orderStatus` and `historicalOrders`, and what
/// the socket's `openOrders` and `userHistoricalOrders` send: an `openOrders` row with its
/// trigger and what kind of order it is.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendOrder {
    pub coin: String,
    pub side: Side,
    pub limit_px: Dec,
    /// What is left; `0` on a position TP/SL, which closes the whole position.
    pub sz: Dec,
    pub oid: u64,
    /// Milliseconds.
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
    /// As the venue writes it: `Gtc`, `Alo`, `Ioc`, `FrontendMarket`, `LiquidationMarket`;
    /// `null` on a trigger still waiting.
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

/// Where an order stands, by the kind of end the venue's word names. The word itself is kept on
/// a cancel or a rejection: `marginCanceled`, `scheduledCancel` (the dead man's switch),
/// `minTradeNtlRejected`.
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

/// An order and where it stood at `status_timestamp`: an `orderStatus` answer's order, each
/// `historicalOrders` row, and each order `userHistoricalOrders` sends.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub order: FrontendOrder,
    pub status: OrderState,
    /// Milliseconds.
    pub status_timestamp: u64,
}
