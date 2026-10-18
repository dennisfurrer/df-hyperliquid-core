//! Orders: the wire every order type ends in ([`Order`], [`PlaceOrder`]), what an order's `t`
//! says, its client id, and one module per type. Keys go in the order the venue hashes:
//! `a b p s r t c` for an order, then `grouping`, then `builder` last.

pub mod cloid;
pub mod close;
pub mod entry;
pub mod limit;
pub mod market;
pub mod order_type;
pub mod scale;
pub mod tpsl;
pub mod trigger;
pub mod twap;

pub use cloid::Cloid;
pub use close::{Close, close};
pub use entry::{OrderIntent, Price, order};
pub use order_type::{Grouping, OrderType, Tif, Tpsl};
pub use scale::{LEAST_LEVELS, MOST_LEVELS, Scale, scale};
pub use tpsl::{Covers, TriggerIntent, trigger};
pub use trigger::{Exit, ExitPrice, TRIGGER_SLIPPAGE};
pub use twap::{MINIMUM_TWAP_VALUE, Twap, TwapCancel, TwapIntent, twap};

use crate::trade::builder::{Builder, OverCap};
use crate::utils::sign::{L1Action, Map, Value};
use crate::utils::{Asset, AssetKind, Dec};

/// Which way an order trades, the same `B` and `A` the reads and the socket carry.
pub use crate::markets::Side;

/// One order on any market. Price and size go on the wire exactly as given,
/// so put them on the asset's grid first ([`crate::utils::Grid`]).
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

    /// `a b p s r t c`, in the order the venue hashes; `c` only when there is one.
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
        let fee = self.builder?.fee();
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
#[path = "tests/order/mod.rs"]
mod tests;
