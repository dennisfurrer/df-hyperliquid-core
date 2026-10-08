//! Leverage and isolated margin on a main-dex perp, checked against what
//! `meta` says the asset allows before anything is signed.

use hl_core::{Asset, BadUnits, Dec};
use hl_trade::{MarginMode, Side, TradeError, UpdateIsolatedMargin, UpdateLeverage};

use crate::meta::PerpAsset;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeverageError {
    #[error("{name} allows at most {max}x, not {asked}x")]
    TooHigh { name: String, max: u32, asked: u32 },
    #[error("{0} is isolated only")]
    NoCross(String),
    #[error(transparent)]
    Trade(#[from] TradeError),
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
