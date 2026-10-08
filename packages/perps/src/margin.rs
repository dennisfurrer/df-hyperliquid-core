//! Leverage and isolated margin on a main-dex perp, checked against what
//! `meta` says the asset allows, and against its margin table, before
//! anything is signed.

use hl_core::{Asset, BadUnits, Dec};
use hl_trade::{MarginMode, Side, TradeError, UpdateIsolatedMargin, UpdateLeverage};

use crate::{meta::PerpAsset, table::MarginTable};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeverageError {
    #[error("{name} allows at most {max}x, not {asked}x")]
    TooHigh { name: String, max: u32, asked: u32 },
    #[error("{name} allows at most {max}x at a notional of {notional}, not {asked}x")]
    OverTier { name: String, notional: Dec, max: u32, asked: u32 },
    #[error("{0} is isolated only")]
    NoCross(String),
    #[error(transparent)]
    Trade(#[from] TradeError),
}

/// [`update_leverage`], and held to the tier of `table` (the perp's, from
/// [`crate::Meta::table_of`]) that a position of `notional` reaches: BTC
/// takes 40x up to 150M and 20x past it.
pub fn update_leverage_for(
    asset: Asset,
    perp: &PerpAsset,
    table: &MarginTable,
    mode: MarginMode,
    leverage: u32,
    notional: Dec,
) -> Result<UpdateLeverage, LeverageError> {
    let set = update_leverage(asset, perp, mode, leverage)?;
    match table.max_leverage_at(notional) {
        Some(max) if leverage > max => {
            Err(LeverageError::OverTier { name: perp.name.clone(), notional: notional.abs(), max, asked: leverage })
        }
        _ => Ok(set),
    }
}

/// `updateLeverage` for `perp` at `asset`: refused over its `maxLeverage`,
/// and refused as cross where it is isolated only.
pub fn update_leverage(
    asset: Asset,
    perp: &PerpAsset,
    mode: MarginMode,
    leverage: u32,
) -> Result<UpdateLeverage, LeverageError> {
    if leverage > perp.max_leverage {
        return Err(LeverageError::TooHigh { name: perp.name.clone(), max: perp.max_leverage, asked: leverage });
    }
    if mode == MarginMode::Cross && !perp.allows_cross() {
        return Err(LeverageError::NoCross(perp.name.clone()));
    }
    Ok(UpdateLeverage::new(asset, mode, leverage)?)
}

/// More USDC into an isolated position, long (`Side::Buy`) or short.
pub fn add_margin(asset: Asset, position: Side, usdc: Dec) -> Result<UpdateIsolatedMargin, TradeError> {
    match usdc.is_positive() {
        true => UpdateIsolatedMargin::new(asset, position, usdc),
        false => Err(TradeError::Margin(BadUnits::NotPositive(usdc))),
    }
}

/// USDC out of an isolated position: sent as a negative `ntli`.
pub fn remove_margin(asset: Asset, position: Side, usdc: Dec) -> Result<UpdateIsolatedMargin, TradeError> {
    match usdc.is_positive() {
        true => UpdateIsolatedMargin::new(asset, position, usdc.negate()),
        false => Err(TradeError::Margin(BadUnits::NotPositive(usdc))),
    }
}
