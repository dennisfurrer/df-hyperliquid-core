//! An entry: one order at market or at a limit, with its take profit and stop loss grouped
//! `normalTpsl` when it has them. Each order is worth at least [`MINIMUM_ORDER_VALUE`].

use crate::trade::builder::BuilderSetting;
use crate::trade::orders::trigger::Exit;
use crate::trade::orders::{Cloid, Grouping, Order, PlaceOrder, Side, Tif, limit, market, tpsl};
use crate::trade::ticket::{Amount, Market, Refusal, at_least, routed};
use crate::utils::Dec;
use crate::utils::rounding::MINIMUM_ORDER_VALUE;

/// At what price an order goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Price {
    /// `FrontendMarket`, bounded `slippage` (a fraction, 0.01 for 1%) past
    /// `reference`: the mid, or else the mark.
    Market { reference: Dec, slippage: Dec },
    /// A limit at `price`, resting as `tif` says. Never `FrontendMarket`.
    Limit { price: Dec, tif: Tif },
}

/// An order to place: a market or limit entry, with its take profit and stop
/// loss when it has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderIntent {
    pub side: Side,
    pub amount: Amount,
    pub price: Price,
    pub reduce_only: bool,
    pub cloid: Option<Cloid>,
    pub take_profit: Option<Exit>,
    pub stop_loss: Option<Exit>,
}

impl OrderIntent {
    pub(super) const fn at(side: Side, amount: Amount, price: Price) -> OrderIntent {
        OrderIntent { side, amount, price, reduce_only: false, cloid: None, take_profit: None, stop_loss: None }
    }

    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> OrderIntent {
        self.cloid = Some(cloid);
        self
    }

    /// Placed with the entry, the other way, once it fills.
    #[must_use]
    pub const fn take_profit(mut self, exit: Exit) -> OrderIntent {
        self.take_profit = Some(exit);
        self
    }

    /// Placed with the entry, the other way, once it fills.
    #[must_use]
    pub const fn stop_loss(mut self, exit: Exit) -> OrderIntent {
        self.stop_loss = Some(exit);
        self
    }
}

/// An `order`: the entry, and its take profit and stop loss grouped `normalTpsl` when it has them.
/// Each is worth at least [`MINIMUM_ORDER_VALUE`] in the quote, valued at its limit, at the
/// reference at market, or at the trigger.
pub fn order(market: &Market, intent: &OrderIntent, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    let has_exits = intent.take_profit.is_some() || intent.stop_loss.is_some();
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    if market.is_spot() && has_exits {
        return Err(Refusal::Spot("a take profit or stop loss"));
    }
    let grid = market.grid();
    let side = intent.side;
    let (price, valued_at, tif) = match intent.price {
        Price::Market { reference, slippage } => market::bound(grid, reference, slippage, side)?,
        Price::Limit { price, tif } => limit::resting(grid, price, tif, side)?,
    };
    let size = intent.amount.size_on(grid, valued_at)?;
    at_least(size, valued_at, MINIMUM_ORDER_VALUE)?;
    let mut entry = match tif {
        Tif::FrontendMarket => Order::market(market.asset(), side, price, size),
        tif => Order::limit(market.asset(), side, price, size, tif),
    };
    if intent.reduce_only {
        entry = entry.reduce_only();
    }
    if let Some(cloid) = intent.cloid {
        entry = entry.with_cloid(cloid);
    }
    let mut orders = vec![entry];
    orders.extend(tpsl::attach(market, side, size, valued_at, intent.take_profit, intent.stop_loss)?);
    let grouping = match has_exits {
        true => Grouping::NormalTpsl,
        false => Grouping::Na,
    };
    routed(PlaceOrder::new(orders).grouped(grouping), builder)
}

#[cfg(test)]
#[path = "tests/entry.rs"]
mod tests;
