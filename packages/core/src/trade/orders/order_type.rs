//! What an order's `t` says and how the orders of one action relate: a limit's time in force, or a
//! trigger and which way it fires, and the action's grouping.

use crate::utils::Dec;
use crate::utils::sign::Map;

/// How long an order may rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Tif {
    /// Add liquidity only: refused if it would cross.
    Alo,
    /// Immediate or cancel.
    Ioc,
    /// Rests until filled or cancelled. Never a market order.
    Gtc,
    /// The venue's market order: crosses up to its price and never rests. Only
    /// [`crate::trade::Order::market`] makes one.
    FrontendMarket,
}

impl Tif {
    /// As `tif` writes it: `Alo`, `Ioc`, `Gtc` or `FrontendMarket`.
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
#[non_exhaustive]
pub enum Tpsl {
    /// When the price moves the position's way.
    TakeProfit,
    /// When it moves against it.
    StopLoss,
}

impl Tpsl {
    /// As `tpsl` writes it: `tp` or `sl`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Tpsl::TakeProfit => "tp",
            Tpsl::StopLoss => "sl",
        }
    }
}

/// How the orders of one action relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Grouping {
    /// Each order stands alone.
    #[default]
    Na,
    /// An entry and its take profit and stop loss.
    NormalTpsl,
    /// A take profit or stop loss on the whole position.
    PositionTpsl,
}

impl Grouping {
    /// As `grouping` writes it: `na`, `normalTpsl` or `positionTpsl`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Grouping::Na => "na",
            Grouping::NormalTpsl => "normalTpsl",
            Grouping::PositionTpsl => "positionTpsl",
        }
    }
}

/// What an order's `t` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OrderType {
    /// `limit`: rests or crosses as its time in force says.
    Limit(Tif),
    /// Rests off the book until the mark crosses `trigger`; then a market
    /// order bounded by the order's price (`is_market`), or a limit at it.
    Trigger {
        /// `isMarket`: a market order once it fires, else a limit.
        is_market: bool,
        /// `triggerPx`: the mark price that fires it.
        trigger: Dec,
        /// Take profit or stop loss.
        tpsl: Tpsl,
    },
}

impl OrderType {
    pub(super) fn wire(self) -> Map {
        match self {
            OrderType::Limit(tif) => Map::new().put("limit", Map::new().put("tif", tif.wire())),
            OrderType::Trigger { is_market, trigger, tpsl } => Map::new().put(
                "trigger",
                Map::new().put("isMarket", is_market).put("triggerPx", trigger.to_wire()).put("tpsl", tpsl.wire()),
            ),
        }
    }
}
