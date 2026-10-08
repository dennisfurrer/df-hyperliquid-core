//! Margin tables: how much leverage a position may take at each size. A
//! perp names its table by `marginTableId`; `meta` lists the tiered ones
//! under `marginTables`, and an id under [`FLAT_TABLES_BELOW`] is a single
//! tier at that leverage, never listed. Hyperliquid's docs (trading/
//! margin-tiers) give BTC 40x up to 150M of notional and 20x past it, which
//! is table 56 as `meta` lists it.

use hl_core::Dec;
use serde::Deserialize;

use crate::meta::{Meta, PerpAsset};

/// A table id under this is a flat table at that leverage.
pub const FLAT_TABLES_BELOW: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginTable {
    /// `tiered 40x`; empty on some.
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
    /// One tier, from nothing on: what an id under [`FLAT_TABLES_BELOW`] is.
    #[must_use]
    pub fn flat(max_leverage: u32) -> MarginTable {
        MarginTable {
            description: String::new(),
            margin_tiers: vec![MarginTier { lower_bound: Dec::ZERO, max_leverage }],
        }
    }

    /// The most leverage a position of `notional` may take: the tier with the
    /// highest lower bound at or under it. A short's notional counts as its
    /// size. The docs do not say which tier a notional exactly on a bound
    /// falls in; here it is the higher one, the bound being its lower.
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

impl Meta {
    /// A table by its id: listed, or flat under [`FLAT_TABLES_BELOW`]. No
    /// perp is on table 0.
    #[must_use]
    pub fn margin_table(&self, id: u32) -> Option<MarginTable> {
        if id == 0 {
            return None;
        }
        if id < FLAT_TABLES_BELOW {
            return Some(MarginTable::flat(id));
        }
        self.margin_tables.iter().find(|(listed, _)| *listed == id).map(|(_, table)| table.clone())
    }

    /// The table `perp` is on. A perp that names none is held to its
    /// `maxLeverage` at any size.
    #[must_use]
    pub fn table_of(&self, perp: &PerpAsset) -> Option<MarginTable> {
        match perp.margin_table_id {
            Some(id) => self.margin_table(id),
            None => Some(MarginTable::flat(perp.max_leverage)),
        }
    }
}
