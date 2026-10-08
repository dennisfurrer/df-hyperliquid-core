//! Leverage and isolated margin on a main-dex perp, checked against what
//! `meta` says the asset allows, and against its margin table, before
//! anything is signed.

use hl_core::{Asset, BadUnits, Dec};
/// Leverage refusals are hl-trade's, shared with hl-hip3.
pub use hl_trade::LeverageError;
use hl_trade::{MarginMode, Side, TradeError, UpdateIsolatedMargin, UpdateLeverage, checked_leverage};

use crate::{meta::PerpAsset, table::MarginTable};

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
    table.check(&perp.name, leverage, notional)?;
    Ok(set)
}

/// `updateLeverage` for `perp` at `asset`: refused over its `maxLeverage`,
/// and refused as cross where it is isolated only.
pub fn update_leverage(
    asset: Asset,
    perp: &PerpAsset,
    mode: MarginMode,
    leverage: u32,
) -> Result<UpdateLeverage, LeverageError> {
    checked_leverage(asset, &perp.name, perp.max_leverage, perp.allows_cross(), mode, leverage)
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
