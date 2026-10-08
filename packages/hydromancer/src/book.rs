//! One market's L4 book: every resting order with its owner, in queue order, keyed by oid.
//!
//! - **Price to queue**: a sorted map a side, the touch at one end, each price's oids front of
//!   the queue first.
//! - **Oid to order**: an update or a remove, most of what a block holds, is a lookup.
//! - **Owner to oids**: one address's orders without walking the book.
//!
//! **Matching is not optional.** The stream reports orders arriving, not trades clearing: an
//! order that crosses arrives as `new`, and the resting orders it took change in that block or a
//! later one. So an insert crosses first and only what is left rests, or the book soon shows a
//! bid above the ask. The venue's own updates and removes that follow are idempotent against it:
//! an update names the size that rests, and a remove of an order already taken does nothing.

use std::collections::{BTreeMap, BTreeSet};

use hl_core::{Address, Dec};

use crate::{
    snapshot::{Market, Order, Side},
    socket::Diff,
};

/// One price on one side, as an L2 book shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub px: Dec,
    /// Every order's size at this price, added.
    pub sz: Dec,
    pub orders: usize,
}

/// What an arriving order took on its way in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Crossed {
    /// How much of it traded and never rested.
    pub filled: Dec,
    /// Resting orders it took whole, as they were.
    pub taken: Vec<Order>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Book {
    orders: BTreeMap<u64, Order>,
    bids: BTreeMap<Dec, Vec<u64>>,
    asks: BTreeMap<Dec, Vec<u64>>,
    owners: BTreeMap<Address, BTreeSet<u64>>,
}

impl Book {
    #[must_use]
    pub fn new() -> Book {
        Book::default()
    }

    /// A snapshot's market, each side's orders queued in the order listed.
    #[must_use]
    pub fn from_market(market: &Market) -> Book {
        let mut book = Book::new();
        for order in market.bids.iter().chain(&market.asks) {
            book.insert(order.clone(), None);
        }
        book
    }

    /// Resting orders, both sides.
    #[must_use]
    pub fn len(&self) -> usize {
        self.orders.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    #[must_use]
    pub fn get(&self, oid: u64) -> Option<&Order> {
        self.orders.get(&oid)
    }

    #[must_use]
    pub fn best_bid(&self) -> Option<Dec> {
        self.bids.keys().next_back().copied()
    }

    #[must_use]
    pub fn best_ask(&self) -> Option<Dec> {
        self.asks.keys().next().copied()
    }

    /// The best price on `side`.
    #[must_use]
    pub fn best(&self, side: Side) -> Option<Dec> {
        match side {
            Side::Bid => self.best_bid(),
            Side::Ask => self.best_ask(),
        }
    }

    /// The best ask less the best bid; none while a side is empty.
    #[must_use]
    pub fn spread(&self) -> Option<Dec> {
        self.best_ask()?.checked_sub(self.best_bid()?)
    }

    /// One side's prices from the touch outward, each with its queue.
    fn touch_out(&self, side: Side) -> Box<dyn Iterator<Item = (&Dec, &Vec<u64>)> + '_> {
        match side {
            Side::Bid => Box::new(self.bids.iter().rev()),
            Side::Ask => Box::new(self.asks.iter()),
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut BTreeMap<Dec, Vec<u64>> {
        match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        }
    }

    /// One side's orders from the touch outward, each queue front first.
    pub fn orders(&self, side: Side) -> impl Iterator<Item = &Order> {
        self.touch_out(side).flat_map(|(_, queue)| queue.iter().filter_map(|oid| self.orders.get(oid)))
    }

    /// The queue at one price, front first.
    pub fn queue(&self, side: Side, px: Dec) -> impl Iterator<Item = &Order> {
        let queue = match side {
            Side::Bid => self.bids.get(&px),
            Side::Ask => self.asks.get(&px),
        };
        queue.into_iter().flatten().filter_map(|oid| self.orders.get(oid))
    }

    /// One owner's resting orders, both sides, by oid.
    pub fn orders_of(&self, user: Address) -> impl Iterator<Item = &Order> {
        self.owners.get(&user).into_iter().flatten().filter_map(|oid| self.orders.get(oid))
    }

    /// One side as L2 levels, from the touch outward.
    #[must_use]
    pub fn levels(&self, side: Side) -> Vec<Level> {
        self.touch_out(side)
            .map(|(&px, queue)| Level {
                px,
                sz: queue
                    .iter()
                    .filter_map(|oid| self.orders.get(oid))
                    .fold(Dec::ZERO, |sum, order| plus(sum, order.sz)),
                orders: queue.len(),
            })
            .collect()
    }

    /// Adds an arriving order, crossing it against the other side first. A maker that paid for
    /// priority (ALO) joins in front of `insert_before` when that order rests at the same price,
    /// else at the back. `None` for an oid the book already holds: a redelivery, not a second
    /// arrival. A size or price at or below zero rests nothing.
    pub fn insert(&mut self, mut order: Order, insert_before: Option<u64>) -> Option<Crossed> {
        if self.orders.contains_key(&order.oid) {
            return None;
        }
        let crossed = self.cross(&mut order);
        if !order.sz.is_positive() || !order.px.is_positive() {
            return Some(crossed);
        }
        let (oid, side, px) = (order.oid, order.side, order.px);
        let ahead = insert_before
            .and_then(|target| self.orders.get(&target))
            .filter(|target| target.side == side && target.px == px)
            .map(|target| target.oid);
        let queue = self.side_mut(side).entry(px).or_default();
        match ahead.and_then(|target| queue.iter().position(|&queued| queued == target)) {
            Some(at) => queue.insert(at, oid),
            None => queue.push(oid),
        }
        self.owners.entry(order.user).or_default().insert(oid);
        self.orders.insert(oid, order);
        Some(crossed)
    }

    /// A resting order's size is now `sz` (a partial fill); at or below zero it goes. False for
    /// an order the book does not hold.
    pub fn resize(&mut self, oid: u64, sz: Dec) -> bool {
        if !sz.is_positive() {
            return self.remove(oid).is_some();
        }
        match self.orders.get_mut(&oid) {
            Some(order) => {
                order.sz = sz;
                true
            }
            None => false,
        }
    }

    /// Takes an order off the book, and gives it back.
    pub fn remove(&mut self, oid: u64) -> Option<Order> {
        let order = self.orders.remove(&oid)?;
        let side = self.side_mut(order.side);
        if let Some(queue) = side.get_mut(&order.px) {
            queue.retain(|&queued| queued != oid);
            if queue.is_empty() {
                side.remove(&order.px);
            }
        }
        if let Some(oids) = self.owners.get_mut(&order.user) {
            oids.remove(&oid);
            if oids.is_empty() {
                self.owners.remove(&order.user);
            }
        }
        Some(order)
    }

    /// One change from the stream. A diff naming another coin is the caller's to route.
    pub fn apply(&mut self, diff: &Diff) {
        match diff {
            Diff::New { order, insert_before, .. } => {
                self.insert(order.clone(), *insert_before);
            }
            Diff::Update { oid, sz, .. } => {
                self.resize(*oid, *sz);
            }
            Diff::Remove { oid, .. } => {
                self.remove(*oid);
            }
        }
    }

    /// Crosses `order` against the other side, the touch first and each queue front first,
    /// cutting its size as it goes.
    fn cross(&mut self, order: &mut Order) -> Crossed {
        let mut crossed = Crossed::default();
        let other = order.side.other();
        while order.sz.is_positive() {
            let Some(px) = self.best(other) else { break };
            let crosses = match order.side {
                Side::Bid => px <= order.px,
                Side::Ask => px >= order.px,
            };
            if !crosses {
                break;
            }
            let Some(maker) = self.queue(other, px).next().cloned() else { break };
            let take = order.sz.min(maker.sz);
            order.sz = minus(order.sz, take);
            crossed.filled = plus(crossed.filled, take);
            match take >= maker.sz {
                true => {
                    self.remove(maker.oid);
                    crossed.taken.push(maker);
                }
                false => {
                    self.resize(maker.oid, minus(maker.sz, take));
                }
            }
        }
        crossed
    }
}

/// `a + b`. A book's sizes are far inside what a [`Dec`] holds; past it the sum stays `a`.
fn plus(a: Dec, b: Dec) -> Dec {
    a.checked_add(b).unwrap_or(a)
}

/// `a - b`, for `b` no larger than `a`.
fn minus(a: Dec, b: Dec) -> Dec {
    a.checked_sub(b).unwrap_or(Dec::ZERO)
}

/// Orders added up into L2 levels for `side`, from the touch outward: a snapshot's market, or any
/// list of orders, without building a book. Orders on the other side are left out.
#[must_use]
pub fn levels<'a>(orders: impl IntoIterator<Item = &'a Order>, side: Side) -> Vec<Level> {
    let mut prices: BTreeMap<Dec, (Dec, usize)> = BTreeMap::new();
    for order in orders.into_iter().filter(|order| order.side == side) {
        let level = prices.entry(order.px).or_insert((Dec::ZERO, 0));
        *level = (plus(level.0, order.sz), level.1 + 1);
    }
    let level = |(px, (sz, orders)): (Dec, (Dec, usize))| Level { px, sz, orders };
    match side {
        Side::Bid => prices.into_iter().rev().map(level).collect(),
        Side::Ask => prices.into_iter().map(level).collect(),
    }
}

#[cfg(test)]
#[path = "tests/book.rs"]
mod tests;
