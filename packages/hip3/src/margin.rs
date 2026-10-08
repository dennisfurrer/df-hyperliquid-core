//! Margin tables and leverage on a HIP-3 perp, checked before anything is
//! signed, as `hl-perps` checks them on the main dex. A deployer sets each
//! perp's table (`setMarginTableIds`): an id under [`FLAT_TABLES_BELOW`] is
//! a single tier at that leverage, never listed; a listed one has up to
//! three tiers.

use hl_core::{Asset, Dec};
/// The tables and the refusals are hl-trade's, shared with hl-perps.
pub use hl_trade::{FLAT_TABLES_BELOW, LeverageError, MarginTable, MarginTier};
use hl_trade::{MarginMode, UpdateLeverage, checked_leverage};

use crate::dex::{DexAsset, DexMeta};

impl DexMeta {
    /// A table by its id: listed, or flat under [`FLAT_TABLES_BELOW`]. No
    /// perp is on table 0.
    #[must_use]
    pub fn margin_table(&self, id: u32) -> Option<MarginTable> {
        MarginTable::by_id(id, &self.margin_tables)
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

/// `updateLeverage` for `perp` at `asset`: refused over its `maxLeverage`,
/// and refused as cross where the deployer made it isolated only.
pub fn update_leverage(
    asset: Asset,
    perp: &DexAsset,
    mode: MarginMode,
    leverage: u32,
) -> Result<UpdateLeverage, LeverageError> {
    checked_leverage(asset, &perp.name, perp.max_leverage, perp.allows_cross(), mode, leverage)
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
    table.check(&perp.name, leverage, notional)?;
    Ok(set)
}
