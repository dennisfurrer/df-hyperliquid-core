//! `updateLeverage`: the leverage and margin mode of a perp position, main dex or HIP-3.

use crate::trade::error::TradeError;
use crate::trade::ticket::{Market, Refusal};
use crate::utils::Asset;
use crate::utils::sign::{L1Action, Map};

/// How a position is margined, the same mode a position read back carries.
pub use crate::account::clearinghouse::MarginMode;

/// `updateLeverage`: `{type, asset, isCross, leverage}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateLeverage {
    asset: Asset,
    mode: MarginMode,
    leverage: u32,
}

impl UpdateLeverage {
    /// A whole number, at least 1. What the asset allows (its `maxLeverage`, whether it takes cross)
    /// is checked by [`crate::trade::checked_leverage`].
    pub fn new(asset: Asset, mode: MarginMode, leverage: u32) -> Result<UpdateLeverage, TradeError> {
        match leverage {
            0 => Err(TradeError::NoLeverage),
            leverage => Ok(UpdateLeverage { asset, mode, leverage }),
        }
    }

    #[must_use]
    pub const fn leverage(&self) -> u32 {
        self.leverage
    }

    #[must_use]
    pub const fn mode(&self) -> MarginMode {
        self.mode
    }
}

impl L1Action for UpdateLeverage {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "updateLeverage")
            .put("asset", self.asset)
            .put("isCross", matches!(self.mode, MarginMode::Cross))
            .put("leverage", self.leverage)
    }
}

/// `updateLeverage`, held to the market's ceiling and margin mode.
pub fn leverage(market: &Market, mode: MarginMode, leverage: u32) -> Result<UpdateLeverage, Refusal> {
    let Some(most) = market.max_leverage() else {
        return Err(Refusal::Spot("leverage"));
    };
    if leverage == 0 || leverage > most {
        return Err(Refusal::Leverage { asked: leverage, most });
    }
    if mode == MarginMode::Cross && !market.allows_cross() {
        return Err(Refusal::IsolatedOnly);
    }
    Ok(UpdateLeverage::new(market.asset(), mode, leverage)?)
}

#[cfg(test)]
#[path = "tests/leverage.rs"]
mod tests;
