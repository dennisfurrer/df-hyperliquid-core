//! Market orders: `FrontendMarket` at the reference (the mid, or else the mark) moved by the
//! slippage, on the grid. Never `Gtc`, which would rest whatever did not fill.

use crate::trade::orders::entry::{OrderIntent, Price};
use crate::trade::orders::{Side, Tif};
use crate::trade::ticket::{Amount, Refusal, positive};
use crate::utils::Dec;
use crate::utils::rounding::Grid;

impl OrderIntent {
    #[must_use]
    pub const fn market(side: Side, amount: Amount, reference: Dec, slippage: Dec) -> OrderIntent {
        OrderIntent::at(side, amount, Price::Market { reference, slippage })
    }
}

/// A market order's price (its bound past `reference`), what it is valued at (`reference`) and
/// its time in force.
pub(crate) fn bound(grid: Grid, reference: Dec, slippage: Dec, side: Side) -> Result<(Dec, Dec, Tif), Refusal> {
    let reference = positive(reference, "a reference price")?;
    Ok((grid.market_bound(reference, slippage, side)?, reference, Tif::FrontendMarket))
}

#[cfg(test)]
#[path = "tests/market.rs"]
mod tests;
