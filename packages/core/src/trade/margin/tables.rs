//! Margin tables and the leverage a perp allows, checked before anything is signed. A perp names
//! its table by `marginTableId`; an id under [`FLAT_TABLES_BELOW`] is a single tier at that
//! leverage, never listed. The main dex and every HIP-3 dex read them the same way.

use crate::trade::error::TradeError;
use crate::trade::margin::{MarginMode, UpdateLeverage};
use crate::utils::{Asset, Dec};
use serde::Deserialize;

/// A table id under this is a flat table at that leverage.
pub const FLAT_TABLES_BELOW: u32 = 50;

/// A margin table, as a meta's `marginTables` lists it: the most leverage by position notional.
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
    /// `maxLeverage`: the most leverage from `lower_bound` on.
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

    /// A table by its id: flat under [`FLAT_TABLES_BELOW`], else one of
    /// `listed`, a meta's `marginTables`. No perp is on table 0.
    #[must_use]
    pub fn by_id(id: u32, listed: &[(u32, MarginTable)]) -> Option<MarginTable> {
        match id {
            0 => None,
            id if id < FLAT_TABLES_BELOW => Some(MarginTable::flat(id)),
            id => listed.iter().find(|(at, _)| *at == id).map(|(_, table)| table.clone()),
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

    /// `leverage` on the perp `name`, refused over what the tier a position
    /// of `notional` reaches allows.
    pub fn check(&self, name: &str, leverage: u32, notional: Dec) -> Result<(), LeverageError> {
        match self.max_leverage_at(notional) {
            Some(max) if leverage > max => {
                Err(LeverageError::OverTier { name: name.to_owned(), notional: notional.abs(), max, asked: leverage })
            }
            _ => Ok(()),
        }
    }
}

/// Why a leverage is not one the perp allows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeverageError {
    /// Over the perp's `maxLeverage`.
    #[error("{name} allows at most {max}x, not {asked}x")]
    TooHigh {
        /// The perp's name.
        name: String,
        /// Its `maxLeverage`.
        max: u32,
        /// The leverage asked for.
        asked: u32,
    },
    /// Over what the margin tier a position of this notional reaches allows.
    #[error("{name} allows at most {max}x at a notional of {notional}, not {asked}x")]
    OverTier {
        /// The perp's name.
        name: String,
        /// The position's notional, unsigned, in the dex's collateral.
        notional: Dec,
        /// The most leverage the tier allows.
        max: u32,
        /// The leverage asked for.
        asked: u32,
    },
    /// The perp is isolated only, and cross was asked for; the perp's name.
    #[error("{0} is isolated only")]
    NoCross(String),
    /// A check on the action itself failed: a leverage of 0.
    #[error(transparent)]
    Trade(#[from] TradeError),
}

/// `updateLeverage` at `asset` for the perp `name`: refused over `max`, its `maxLeverage`, and
/// refused as cross where `cross` is false, the perp being isolated only. Both come from the
/// perp's meta.
pub fn checked_leverage(
    asset: Asset,
    name: &str,
    max: u32,
    cross: bool,
    mode: MarginMode,
    leverage: u32,
) -> Result<UpdateLeverage, LeverageError> {
    if leverage > max {
        return Err(LeverageError::TooHigh { name: name.to_owned(), max, asked: leverage });
    }
    if mode == MarginMode::Cross && !cross {
        return Err(LeverageError::NoCross(name.to_owned()));
    }
    Ok(UpdateLeverage::new(asset, mode, leverage)?)
}

#[cfg(test)]
#[path = "tests/tables.rs"]
mod tests;
