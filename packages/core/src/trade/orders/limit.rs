//! Limit orders: `Gtc`, `Ioc` or `Alo` at a price brought onto the grid, never against the
//! trader. A limit is never `FrontendMarket`.

use crate::trade::error::TradeError;
use crate::trade::orders::entry::{OrderIntent, Price};
use crate::trade::orders::{Side, Tif};
use crate::trade::ticket::{Amount, Refusal, positive};
use crate::utils::Dec;
use crate::utils::rounding::Grid;

impl OrderIntent {
    #[must_use]
    pub const fn limit(side: Side, amount: Amount, price: Dec, tif: Tif) -> OrderIntent {
        OrderIntent::at(side, amount, Price::Limit { price, tif })
    }
}

/// A limit order's price on the grid, what it is valued at (the same price) and its time in force.
pub(crate) fn resting(grid: Grid, price: Dec, tif: Tif, side: Side) -> Result<(Dec, Dec, Tif), Refusal> {
    if matches!(tif, Tif::FrontendMarket) {
        return Err(Refusal::MarketTif);
    }
    let price = limit_on(grid, price, side, "a price")?;
    Ok((price, price, tif))
}

/// A limit on the grid for `side`, never against the trader.
pub(crate) fn limit_on(grid: Grid, price: Dec, side: Side, what: &'static str) -> Result<Dec, Refusal> {
    let price = positive(price, what)?;
    grid.price(price, side).ok_or(Refusal::Trade(TradeError::NoPrice(price)))
}

#[cfg(test)]
#[path = "tests/limit.rs"]
mod tests;
