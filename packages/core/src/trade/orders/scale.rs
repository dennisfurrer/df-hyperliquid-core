//! Scale orders: resting limits evenly spaced between two prices, the size split evenly between
//! them, in one action.

use crate::trade::builder::BuilderSetting;
use crate::trade::error::TradeError;
use crate::trade::orders::limit::limit_on;
use crate::trade::orders::{Order, PlaceOrder, Side, Tif};
use crate::trade::ticket::{Market, Refusal, at_least, positive, routed};
use crate::utils::rounding::MINIMUM_ORDER_VALUE;
use crate::utils::{Dec, Round};

/// The fewest levels a scale has.
pub const LEAST_LEVELS: u32 = 2;

/// The most levels a scale has.
pub const MOST_LEVELS: u32 = 50;

/// A scale: `levels` resting orders evenly spaced from `from` to `to`, the
/// size split evenly between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// Buy or sell.
    pub side: Side,
    /// The total, in the base.
    pub size: Dec,
    /// The first level's price.
    pub from: Dec,
    /// The last level's price.
    pub to: Dec,
    /// How many orders, [`LEAST_LEVELS`] to [`MOST_LEVELS`].
    pub levels: u32,
    /// `Gtc` or `Alo`: a scale rests on the book.
    pub tif: Tif,
    /// Whether its orders may only reduce a position.
    pub reduce_only: bool,
}

impl Scale {
    /// `levels` orders from `from` to `to`, not reduce only.
    #[must_use]
    pub const fn new(side: Side, size: Dec, from: Dec, to: Dec, levels: u32, tif: Tif) -> Scale {
        Scale { side, size, from, to, levels, tif, reduce_only: false }
    }
}

/// A scale: one `order` with an order a level. The last level is `to` itself.
pub fn scale(market: &Market, intent: &Scale, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    if !matches!(intent.tif, Tif::Gtc | Tif::Alo) {
        return Err(Refusal::ScaleTif(intent.tif));
    }
    let levels = intent.levels;
    if !(LEAST_LEVELS..=MOST_LEVELS).contains(&levels) {
        return Err(Refusal::Levels(levels));
    }
    let grid = market.grid();
    let total = positive(intent.size, "a size")?;
    let from = positive(intent.from, "a from price")?;
    let to = positive(intent.to, "a to price")?;
    let count = Dec::from_int(i64::from(levels));
    let raw = total.checked_div(count, 12, Round::Down).ok_or(Refusal::Trade(TradeError::NoSize(total)))?;
    let each = grid.size(raw).ok_or(Refusal::Trade(TradeError::NoSize(raw)))?;
    let span = to.checked_sub(from).ok_or(Refusal::Trade(TradeError::NoPrice(to)))?;
    let step = span
        .checked_div(Dec::from_int(i64::from(levels - 1)), 12, Round::Down)
        .ok_or(Refusal::Trade(TradeError::NoPrice(to)))?;
    let mut orders = Vec::new();
    for at in 0..levels {
        let raw = match at + 1 == levels {
            true => to,
            false => step
                .checked_mul(Dec::from_int(i64::from(at)))
                .and_then(|moved| from.checked_add(moved))
                .ok_or(Refusal::Trade(TradeError::NoPrice(from)))?,
        };
        let price = limit_on(grid, raw, intent.side, "a level's price")?;
        at_least(each, price, MINIMUM_ORDER_VALUE)?;
        let order = Order::limit(market.asset(), intent.side, price, each, intent.tif);
        orders.push(match intent.reduce_only {
            true => order.reduce_only(),
            false => order,
        });
    }
    routed(PlaceOrder::new(orders), builder)
}

#[cfg(test)]
#[path = "tests/scale.rs"]
mod tests;
