//! `modify` and `batchModify`: a resting order replaced in place.

use hl_core::{L1Action, Map, Value};

use crate::order::{Cloid, Order};

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

/// `modify`: `{type, oid, order, a?}`, one order.
///
/// `a` is always place, as in a `batchModify`: the venue's exchange docs give
/// both modifies the flag, refuse a trigger without it, and refuse an action
/// hashed with `a: false`. It is written only when set, last, where the SDK's
/// `modify` schema puts it, and set whenever the new order is a trigger. A
/// `batchModify` entry never carries it: the batch does.
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

/// `batchModify`: `{type, modifies: [{oid, order}], a?}`.
///
/// `a` is always place: the new order goes on whether or not the old one was
/// still there to cancel. Hyperliquid's exchange docs refuse a trigger order
/// in a modify without it and refuse an action hashed with `a: false`, so it
/// is written only when set, last, where the SDK's schema puts it, and set
/// whenever a new order is a trigger.
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
