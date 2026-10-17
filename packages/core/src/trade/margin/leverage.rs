//! `updateLeverage`: the leverage and margin mode of a perp position, main dex or HIP-3, checked
//! against the perp's meta and margin table, or against a ticket's [`Market`], before anything is
//! signed.

use crate::markets::perps::meta::PerpAsset;
use crate::trade::error::TradeError;
use crate::trade::margin::tables::{LeverageError, MarginTable, checked_leverage};
use crate::trade::ticket::{Market, Refusal};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Asset, Dec};

/// How a position is margined, the same mode a position read back carries.
pub use crate::account::holdings::MarginMode;

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

/// `updateLeverage` for `perp` at `asset`: refused over its `maxLeverage`, and refused as cross
/// where it is isolated only.
pub fn update_leverage(
    asset: Asset,
    perp: &PerpAsset,
    mode: MarginMode,
    leverage: u32,
) -> Result<UpdateLeverage, LeverageError> {
    checked_leverage(asset, &perp.name, perp.max_leverage, perp.allows_cross(), mode, leverage)
}

/// [`update_leverage`], and held to the tier of `table` (the perp's, from
/// [`crate::markets::perps::Meta::table_of`]) that a position of `notional` reaches: BTC takes 40x
/// up to 150M and 20x past it.
pub fn update_leverage_for(
    asset: Asset,
    perp: &PerpAsset,
    table: &MarginTable,
    mode: MarginMode,
    leverage: u32,
    notional: Dec,
) -> Result<UpdateLeverage, LeverageError> {
    let set = update_leverage(asset, perp, mode, leverage)?;
    table.check(&perp.name, leverage, notional)?;
    Ok(set)
}

/// `updateLeverage`, held to the market's ceiling and margin mode.
pub fn leverage(market: &Market, mode: MarginMode, leverage: u32) -> Result<UpdateLeverage, Refusal> {
    let Some(most) = market.max_leverage() else {
        return Err(Refusal::Spot("leverage"));
    };
    // A ticket's refusals name no perp, so neither does the check.
    checked_leverage(market.asset(), "", most, market.allows_cross(), mode, leverage).map_err(|error| match error {
        LeverageError::TooHigh { .. }
        | LeverageError::OverTier { .. }
        | LeverageError::Trade(TradeError::NoLeverage) => Refusal::Leverage { asked: leverage, most },
        LeverageError::NoCross(_) => Refusal::IsolatedOnly,
        LeverageError::Trade(error) => Refusal::Trade(error),
    })
}

#[cfg(test)]
#[path = "tests/leverage.rs"]
mod tests;
