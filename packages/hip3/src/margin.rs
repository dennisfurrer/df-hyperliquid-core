//! Margin tables and leverage on a HIP-3 perp, checked before anything is
//! signed, as `hl-perps` checks them on the main dex. A deployer sets each
//! perp's table (`setMarginTableIds`): an id under [`FLAT_TABLES_BELOW`] is
//! a single tier at that leverage, never listed; a listed one has up to
//! three tiers.

use hl_core::{Asset, Dec};
use hl_trade::{MarginMode, TradeError, UpdateLeverage};
use serde::Deserialize;

use crate::dex::{DexAsset, DexMeta};

/// A table id under this is a flat table at that leverage.
pub const FLAT_TABLES_BELOW: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginTable {
    pub description: String,
    /// Lowest first.
    pub margin_tiers: Vec<MarginTier>,
}

/// From `lower_bound` of notional on, at most `max_leverage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginTier {
    /// In the dex's collateral.
    pub lower_bound: Dec,
    pub max_leverage: u32,
}

impl MarginTable {
    /// One tier, from nothing on.
    #[must_use]
    pub fn flat(max_leverage: u32) -> MarginTable {
        MarginTable {
            description: String::new(),
            margin_tiers: vec![MarginTier { lower_bound: Dec::ZERO, max_leverage }],
        }
    }

    /// The most leverage a position of `notional` may take: the tier with the
    /// highest lower bound at or under it, a short's notional counted as its
    /// size.
    #[must_use]
    pub fn max_leverage_at(&self, notional: Dec) -> Option<u32> {
        let notional = notional.abs();
        self.margin_tiers
            .iter()
            .filter(|tier| tier.lower_bound <= notional)
            .max_by_key(|tier| tier.lower_bound)
            .map(|tier| tier.max_leverage)
    }
}

impl DexMeta {
    /// A table by its id: listed, or flat under [`FLAT_TABLES_BELOW`]. No
    /// perp is on table 0.
    #[must_use]
    pub fn margin_table(&self, id: u32) -> Option<MarginTable> {
        match id {
            0 => None,
            id if id < FLAT_TABLES_BELOW => Some(MarginTable::flat(id)),
            id => self.margin_tables.iter().find(|(listed, _)| *listed == id).map(|(_, table)| table.clone()),
        }
    }

    /// The table `perp` is on; one that names none is held to its
    /// `maxLeverage` at any size.
    #[must_use]
    pub fn table_of(&self, perp: &DexAsset) -> Option<MarginTable> {
        match perp.margin_table_id {
            Some(id) => self.margin_table(id),
            None => Some(MarginTable::flat(perp.max_leverage)),
        }
    }
}

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

/// `updateLeverage` for `perp` at `asset`: refused over its `maxLeverage`,
/// and refused as cross where the deployer made it isolated only.
pub fn update_leverage(
    asset: Asset,
    perp: &DexAsset,
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

/// [`update_leverage`], and held to the tier of `table` (the perp's, from
/// [`DexMeta::table_of`]) that a position of `notional` reaches.
pub fn update_leverage_for(
    asset: Asset,
    perp: &DexAsset,
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
