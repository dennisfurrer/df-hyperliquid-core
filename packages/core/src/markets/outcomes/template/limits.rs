//! `outcomeDeployerLimits`: how many more outcomes a venue may register today and hold live.

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::Json;
use serde::Deserialize;

/// `{"type":"outcomeDeployerLimits","venue":..}`.
#[must_use]
pub fn deployer_limits_request(venue: &str) -> String {
    info_request("outcomeDeployerLimits", &[("venue", Json::String(venue.to_owned()))])
}

/// What `outcomeDeployerLimits` answers: how many more a venue may register today and how many
/// more it may hold live. Neither cap is documented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[non_exhaustive]
pub struct Remaining {
    /// How many more outcomes the venue may register today (`nDailyOutcomesRemaining`).
    #[serde(default, rename = "nDailyOutcomesRemaining")]
    pub daily: u32,
    /// How many more outcomes the venue may hold live (`nActiveOutcomesRemaining`).
    #[serde(default, rename = "nActiveOutcomesRemaining")]
    pub active: u32,
}

/// How many more outcomes `venue` may register today and hold live.
pub async fn deployer_limits(transport: &impl Transport, venue: &str) -> Result<Remaining, ApiError> {
    info(transport, deployer_limits_request(venue)).await
}

#[cfg(test)]
#[path = "tests/limits.rs"]
mod tests;
