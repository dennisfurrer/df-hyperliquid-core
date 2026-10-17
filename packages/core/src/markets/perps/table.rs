//! Margin tables: how much leverage a position may take at each size. `meta` lists the tiered ones;
//! an id under [`FLAT_TABLES_BELOW`] is a single tier at that leverage and never listed.

use crate::markets::perps::meta::{Meta, PerpAsset};
pub use crate::trade::{FLAT_TABLES_BELOW, MarginTable, MarginTier};

impl Meta {
    /// A table by its id: listed, or flat under [`FLAT_TABLES_BELOW`]. No perp is on table 0.
    #[must_use]
    pub fn margin_table(&self, id: u32) -> Option<MarginTable> {
        MarginTable::by_id(id, &self.margin_tables)
    }

    /// The table `perp` is on. A perp that names none is held to its `maxLeverage` at any size.
    #[must_use]
    pub fn table_of(&self, perp: &PerpAsset) -> Option<MarginTable> {
        match perp.margin_table_id {
            Some(id) => self.margin_table(id),
            None => Some(MarginTable::flat(perp.max_leverage)),
        }
    }
}
