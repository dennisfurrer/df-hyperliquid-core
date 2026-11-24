//! A margin table a HIP-3 deployer inserts: up to three tiers of position size and the leverage
//! allowed above each.

use crate::deploy::error::BadDeploy;
use crate::utils::sign::{Map, Value};

/// The highest max leverage a tier takes.
pub const MAX_LEVERAGE: u32 = 50;
/// The most tiers a table takes.
pub const MAX_TIERS: usize = 3;

/// Above `lower_bound` of position notional, leverage is at most
/// `max_leverage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginTier {
    /// Position notional from which this tier applies (`lowerBound`).
    pub lower_bound: u64,
    /// The highest leverage allowed from the lower bound up (`maxLeverage`), from 1 to 50.
    pub max_leverage: u32,
}

/// `{"description","marginTiers"}`: 1 to 3 tiers, lower bounds rising and
/// max leverage falling, each leverage from 1 to 50.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarginTable {
    description: String,
    tiers: Vec<MarginTier>,
}

impl MarginTable {
    /// Refuses no tiers, more than three, a leverage outside 1 to 50, or
    /// tiers out of order.
    pub fn new(description: impl Into<String>, tiers: Vec<MarginTier>) -> Result<Self, BadDeploy> {
        let in_range = tiers.iter().all(|tier| (1..=MAX_LEVERAGE).contains(&tier.max_leverage));
        let ordered = tiers
            .windows(2)
            .all(|two| two[0].lower_bound < two[1].lower_bound && two[0].max_leverage > two[1].max_leverage);
        if tiers.is_empty() || tiers.len() > MAX_TIERS || !in_range || !ordered {
            return Err(BadDeploy::MarginTiers);
        }
        Ok(MarginTable { description: description.into(), tiers })
    }

    /// The table's description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The tiers, lower bounds rising.
    #[must_use]
    pub fn tiers(&self) -> &[MarginTier] {
        &self.tiers
    }

    /// The table as sent.
    #[must_use]
    pub fn wire(&self) -> Map {
        let tiers = self
            .tiers
            .iter()
            .map(|tier| Map::new().put("lowerBound", tier.lower_bound).put("maxLeverage", tier.max_leverage));
        Map::new().put("description", self.description.clone()).put("marginTiers", Value::array(tiers))
    }
}
