//! `modify` and `batchModify`: a resting order replaced in place.

use crate::account::orders::FrontendOrder;
use crate::trade::orders::Tif;
use crate::trade::orders::limit::limit_on;
use crate::trade::orders::tpsl::Covers;
use crate::trade::orders::trigger::{ExitPrice, fill_price, trigger_on};
use crate::trade::orders::{Cloid, Order};
use crate::trade::ticket::{Amount, Market, Refusal, at_least};
use crate::utils::Dec;
use crate::utils::rounding::MINIMUM_ORDER_VALUE;
use crate::utils::sign::{L1Action, Map, Value};

/// The order a modify replaces: the venue's id, or the client's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderRef {
    Oid(u64),
    Cloid(Cloid),
}

impl OrderRef {
    /// An oid as a number, a cloid as its hex.
    #[must_use]
    pub fn wire(self) -> Value {
        match self {
            OrderRef::Oid(oid) => Value::Uint(oid),
            OrderRef::Cloid(cloid) => Value::from(cloid.to_hex()),
        }
    }
}

/// `modify`: `{type, oid, order, a?}`, one order. `a` (always place) is written last and only
/// when set, and set whenever the new order is a trigger: the venue refuses a trigger without it
/// and an action hashed with `a: false`. A `batchModify` entry never carries it; the batch does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modify {
    target: OrderRef,
    order: Order,
    always_place: bool,
}

impl Modify {
    #[must_use]
    pub const fn new(target: OrderRef, order: Order) -> Modify {
        Modify { target, order, always_place: order.is_trigger() }
    }

    /// Places the new order even where the old one is gone.
    #[must_use]
    pub const fn always_place(mut self) -> Modify {
        self.always_place = true;
        self
    }

    #[must_use]
    pub const fn is_always_place(&self) -> bool {
        self.always_place
    }

    #[must_use]
    pub const fn target(&self) -> OrderRef {
        self.target
    }

    #[must_use]
    pub const fn order(&self) -> &Order {
        &self.order
    }

    /// `{oid, order}`, one entry of a `batchModify`.
    #[must_use]
    pub fn entry(&self) -> Map {
        Map::new().put("oid", self.target.wire()).put("order", self.order.wire())
    }
}

impl L1Action for Modify {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "modify")
            .put("oid", self.target.wire())
            .put("order", self.order.wire())
            .put_some("a", self.always_place.then_some(true))
    }
}

/// `batchModify`: `{type, modifies: [{oid, order}], a?}`. `a` (always place: the new order goes on
/// whether or not the old one was still there) is written last and only when set, and set
/// whenever a new order is a trigger, since the venue refuses a trigger without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchModify {
    modifies: Vec<Modify>,
    always_place: bool,
}

impl BatchModify {
    #[must_use]
    pub fn new(modifies: Vec<Modify>) -> BatchModify {
        let always_place = modifies.iter().any(|modify| modify.order.is_trigger());
        BatchModify { modifies, always_place }
    }

    /// Places the new orders even where the old ones are gone.
    #[must_use]
    pub fn always_place(mut self) -> BatchModify {
        self.always_place = true;
        self
    }

    #[must_use]
    pub fn modifies(&self) -> &[Modify] {
        &self.modifies
    }

    #[must_use]
    pub const fn is_always_place(&self) -> bool {
        self.always_place
    }
}

impl L1Action for BatchModify {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "batchModify")
            .put("modifies", Value::array(self.modifies.iter().map(Modify::entry)))
            .put_some("a", self.always_place.then_some(true))
    }
}

/// What a resting order becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A limit order's new price, size and time in force.
    Limit { price: Dec, size: Amount, tif: Tif },
    /// A take profit's or stop loss's new trigger, price and size.
    Trigger { trigger: Dec, price: ExitPrice, size: Covers },
}

/// A resting order replaced in place, as `frontendOpenOrders` lists it. Its
/// side and reduce only stay; a take profit or stop loss always places, as
/// the venue requires of a trigger.
pub fn modify(market: &Market, order: &FrontendOrder, change: &Change) -> Result<BatchModify, Refusal> {
    let grid = market.grid();
    let side = order.side;
    let new = match *change {
        Change::Limit { .. } if order.is_trigger => return Err(Refusal::NotALimit(order.oid)),
        Change::Limit { tif, .. } if !matches!(tif, Tif::Alo | Tif::Gtc) => return Err(Refusal::ModifyTif(tif)),
        Change::Limit { price, size, tif } => {
            let price = limit_on(grid, price, side, "a new price")?;
            let size = size.size_on(grid, price)?;
            at_least(size, price, MINIMUM_ORDER_VALUE)?;
            Order::limit(market.asset(), side, price, size, tif)
        }
        Change::Trigger { .. } if !order.is_trigger => return Err(Refusal::NotATrigger(order.oid)),
        Change::Trigger { trigger, price, size } => {
            let Some(tpsl) = order.tpsl() else {
                return Err(Refusal::UnknownTrigger { oid: order.oid, kind: order.order_type.word().to_lowercase() });
            };
            let trigger = trigger_on(grid, trigger)?;
            let size = match size {
                Covers::Position if order.sz.is_zero() => Dec::ZERO,
                Covers::Position => return Err(Refusal::NotWholePosition(order.oid)),
                Covers::Size(amount) => {
                    let size = amount.size_on(grid, trigger)?;
                    at_least(size, trigger, MINIMUM_ORDER_VALUE)?;
                    size
                }
            };
            let (price, is_market) = fill_price(grid, trigger, price, side)?;
            Order::trigger(market.asset(), side, price, size, trigger, tpsl, is_market)
        }
    };
    let new = match order.reduce_only {
        true => new.reduce_only(),
        false => new,
    };
    Ok(BatchModify::new(vec![Modify::new(OrderRef::Oid(order.oid), new)]))
}

#[cfg(test)]
#[path = "tests/modify.rs"]
mod tests;
