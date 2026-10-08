//! `order`: one or more orders, grouped, optionally through a builder. Keys
//! go in the order `@nktkas/hyperliquid` 0.33.2 writes them, which is the
//! order the venue hashes: `a b p s r t c` for an order, then `grouping`, then
//! `builder` last.

use std::{fmt, str::FromStr};

use hl_core::{Address, Asset, AssetKind, BadHex, BuilderFee, Dec, L1Action, Map, Value, hex};
use serde::{Deserialize, Deserializer, de};

/// Which way an order trades. It lives in hl-core, where the sockets and the
/// data crates read the same `B` and `A`.
pub use hl_core::Side;

/// How long an order may rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tif {
    /// Add liquidity only: refused if it would cross.
    Alo,
    /// Immediate or cancel.
    Ioc,
    /// Rests until filled or cancelled. Never a market order.
    Gtc,
    /// The venue's market order: crosses up to its price and never rests.
    /// Only [`Order::market`] makes one.
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

/// How the orders of one action relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Grouping::Na => "na",
            Grouping::NormalTpsl => "normalTpsl",
            Grouping::PositionTpsl => "positionTpsl",
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

/// From the hex the venue writes for an order's or a fill's `cloid`.
impl<'de> Deserialize<'de> for Cloid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(de::Error::custom)
    }
}

/// What an order's `t` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit(Tif),
    /// Rests off the book until the mark crosses `trigger`; then a market
    /// order bounded by the order's price (`is_market`), or a limit at it.
    Trigger {
        is_market: bool,
        trigger: Dec,
        tpsl: Tpsl,
    },
}

impl OrderType {
    fn wire(self) -> Map {
        match self {
            OrderType::Limit(tif) => Map::new().put("limit", Map::new().put("tif", tif.wire())),
            OrderType::Trigger { is_market, trigger, tpsl } => Map::new().put(
                "trigger",
                Map::new().put("isMarket", is_market).put("triggerPx", trigger.to_wire()).put("tpsl", tpsl.wire()),
            ),
        }
    }
}

/// One order on any market. Price and size go on the wire exactly as given,
/// so put them on the asset's grid first ([`crate::Grid`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    asset: Asset,
    side: Side,
    price: Dec,
    size: Dec,
    reduce_only: bool,
    order_type: OrderType,
    cloid: Option<Cloid>,
}

impl Order {
    /// A limit order that rests or crosses as `tif` says.
    #[must_use]
    pub const fn limit(asset: Asset, side: Side, price: Dec, size: Dec, tif: Tif) -> Order {
        Order { asset, side, price, size, reduce_only: false, order_type: OrderType::Limit(tif), cloid: None }
    }

    /// A market order: `FrontendMarket`, always, at `bound`, the worst price
    /// the trader takes (the reference moved by the slippage, on the grid; on
    /// an outcome, the edge of the range). Never `Gtc`: a `Gtc` at the bound
    /// would rest on the book for whatever did not fill.
    #[must_use]
    pub const fn market(asset: Asset, side: Side, bound: Dec, size: Dec) -> Order {
        Order::limit(asset, side, bound, size, Tif::FrontendMarket)
    }

    /// A take profit or stop loss: fires when the mark crosses `trigger`, as
    /// a market order bounded by `price` or as a limit at it. A size of zero
    /// on a reduce-only trigger means the whole position.
    #[must_use]
    pub const fn trigger(
        asset: Asset,
        side: Side,
        price: Dec,
        size: Dec,
        trigger: Dec,
        tpsl: Tpsl,
        is_market: bool,
    ) -> Order {
        Order {
            asset,
            side,
            price,
            size,
            reduce_only: false,
            order_type: OrderType::Trigger { is_market, trigger, tpsl },
            cloid: None,
        }
    }

    /// Only ever shrinks the position.
    #[must_use]
    pub const fn reduce_only(mut self) -> Order {
        self.reduce_only = true;
        self
    }

    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> Order {
        self.cloid = Some(cloid);
        self
    }

    #[must_use]
    pub const fn asset(&self) -> Asset {
        self.asset
    }

    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    #[must_use]
    pub const fn price(&self) -> Dec {
        self.price
    }

    #[must_use]
    pub const fn size(&self) -> Dec {
        self.size
    }

    #[must_use]
    pub const fn is_reduce_only(&self) -> bool {
        self.reduce_only
    }

    #[must_use]
    pub const fn order_type(&self) -> OrderType {
        self.order_type
    }

    #[must_use]
    pub const fn cloid(&self) -> Option<Cloid> {
        self.cloid
    }

    /// A limit order's TIF; `None` for a trigger.
    #[must_use]
    pub const fn tif(&self) -> Option<Tif> {
        match self.order_type {
            OrderType::Limit(tif) => Some(tif),
            OrderType::Trigger { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_market(&self) -> bool {
        matches!(self.order_type, OrderType::Limit(Tif::FrontendMarket))
    }

    #[must_use]
    pub const fn is_trigger(&self) -> bool {
        matches!(self.order_type, OrderType::Trigger { .. })
    }

    /// `a b p s r t c`, every client's order; `c` only when there is one.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new()
            .put("a", self.asset)
            .put("b", self.side.is_buy())
            .put("p", self.price.to_wire())
            .put("s", self.size.to_wire())
            .put("r", self.reduce_only)
            .put("t", self.order_type.wire())
            .put_some("c", self.cloid.map(Cloid::to_hex))
    }
}

/// Who routed the order and what it takes: `{b, f}`, the address lowercase,
/// the fee in tenths of a basis point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Builder {
    address: Address,
    fee: BuilderFee,
}

impl Builder {
    /// The account must have approved at least `fee` for this builder
    /// (`approveBuilderFee`), or the venue refuses the order.
    #[must_use]
    pub const fn new(address: Address, fee: BuilderFee) -> Builder {
        Builder { address, fee }
    }

    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    #[must_use]
    pub const fn fee(&self) -> BuilderFee {
        self.fee
    }

    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new().put("b", self.address.to_hex()).put("f", self.fee.tenths())
    }
}

/// A builder fee over what the venue lets a builder take on these orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the builder fee of {} is over the {} {} order allows ({})", .fee.tenths(), .cap.tenths(), .kind.one(), .cap)]
pub struct OverCap {
    pub fee: BuilderFee,
    pub cap: BuilderFee,
    pub kind: AssetKind,
}

/// `order`: one or more orders, grouped, optionally through a builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceOrder {
    orders: Vec<Order>,
    grouping: Grouping,
    builder: Option<Builder>,
}

impl PlaceOrder {
    /// A scale is one action with an order a level.
    #[must_use]
    pub fn new(orders: Vec<Order>) -> PlaceOrder {
        PlaceOrder { orders, grouping: Grouping::Na, builder: None }
    }

    #[must_use]
    pub fn one(order: Order) -> PlaceOrder {
        PlaceOrder::new(vec![order])
    }

    #[must_use]
    pub fn grouped(mut self, grouping: Grouping) -> PlaceOrder {
        self.grouping = grouping;
        self
    }

    #[must_use]
    pub fn through(mut self, builder: Builder) -> PlaceOrder {
        self.builder = Some(builder);
        self
    }

    #[must_use]
    pub fn orders(&self) -> &[Order] {
        &self.orders
    }

    #[must_use]
    pub const fn grouping(&self) -> Grouping {
        self.grouping
    }

    #[must_use]
    pub const fn builder(&self) -> Option<Builder> {
        self.builder
    }

    /// The builder's fee, when it is more than the venue lets a builder take
    /// on these orders. One perp order holds the whole action to the perp cap.
    #[must_use]
    pub fn over_builder_cap(&self) -> Option<OverCap> {
        let fee = self.builder?.fee;
        let kind = AssetKind::strictest(self.orders.iter().map(Order::asset))?;
        let cap = kind.builder_cap();
        (fee > cap).then_some(OverCap { fee, cap, kind })
    }
}

impl L1Action for PlaceOrder {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "order")
            .put("orders", Value::array(self.orders.iter().map(Order::wire)))
            .put("grouping", self.grouping.wire())
            .put_some("builder", self.builder.as_ref().map(Builder::wire))
    }
}

#[cfg(test)]
#[path = "tests/order.rs"]
mod tests;
