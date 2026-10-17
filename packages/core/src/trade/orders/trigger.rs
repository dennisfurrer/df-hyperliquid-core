//! Trigger orders: they rest off the book until the mark crosses the trigger, then fill at
//! market within a slippage or as a limit. Every take profit and stop loss is one.

use crate::trade::error::TradeError;
use crate::trade::orders::limit::limit_on;
use crate::trade::orders::{Order, Side, Tpsl};
use crate::trade::ticket::{Market, Refusal, positive};
use crate::utils::rounding::Grid;
use crate::utils::{Dec, Round};

/// The slippage a market take profit or stop loss takes past its trigger:
/// the venue's TP/SL page gives market TP/SL orders a tolerance of 10%.
pub const TRIGGER_SLIPPAGE: Dec = Dec::lit::<1>(1);

/// How a take profit or stop loss fills once its trigger fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitPrice {
    /// At market, no worse than `slippage` past the trigger.
    Market { slippage: Dec },
    /// A limit at this price.
    Limit(Dec),
}

/// A take profit or stop loss: where it fires and how it fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub trigger: Dec,
    pub price: ExitPrice,
}

impl Exit {
    /// At market, within the venue's own [`TRIGGER_SLIPPAGE`].
    #[must_use]
    pub const fn market(trigger: Dec) -> Exit {
        Exit::market_within(trigger, TRIGGER_SLIPPAGE)
    }

    /// At market, within `slippage`.
    #[must_use]
    pub const fn market_within(trigger: Dec, slippage: Dec) -> Exit {
        Exit { trigger, price: ExitPrice::Market { slippage } }
    }

    /// A limit at `price` once `trigger` fires.
    #[must_use]
    pub const fn limit(trigger: Dec, price: Dec) -> Exit {
        Exit { trigger, price: ExitPrice::Limit(price) }
    }
}

/// A trigger on the grid: to the nearest tick, since where it fires favours
/// neither side.
pub(crate) fn trigger_on(grid: Grid, trigger: Dec) -> Result<Dec, Refusal> {
    let trigger = positive(trigger, "a trigger price")?;
    grid.tick(trigger, Round::Nearest).ok_or(Refusal::Trade(TradeError::NoPrice(trigger)))
}

/// The price a trigger order carries: its bound past the trigger at market,
/// or its limit.
pub(crate) fn fill_price(grid: Grid, trigger: Dec, price: ExitPrice, side: Side) -> Result<(Dec, bool), Refusal> {
    match price {
        ExitPrice::Market { slippage } => Ok((grid.market_bound(trigger, slippage, side)?, true)),
        ExitPrice::Limit(price) => Ok((limit_on(grid, price, side, "a limit price")?, false)),
    }
}

/// A reduce-only trigger closing `size` on `side`.
pub(crate) fn exit_order(market: &Market, side: Side, size: Dec, tpsl: Tpsl, exit: Exit) -> Result<Order, Refusal> {
    let trigger = trigger_on(market.grid(), exit.trigger)?;
    let (price, is_market) = fill_price(market.grid(), trigger, exit.price, side)?;
    Ok(Order::trigger(market.asset(), side, price, size, trigger, tpsl, is_market).reduce_only())
}

#[cfg(test)]
#[path = "tests/trigger.rs"]
mod tests;
