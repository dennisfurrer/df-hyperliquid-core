//! The Dutch auction a deploy slot is bought in, the one shape a spot token, a spot pair and a HIP-3 perp past
//! a dex's first three all answer. Each round is 31 hours; its price is HYPE and is never refunded.

use serde::Deserialize;

use crate::utils::dec::Dec;

/// One round of a deploy auction. Seconds, not milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAuction {
    pub start_time_seconds: u64,
    pub duration_seconds: u64,
    /// HYPE.
    pub start_gas: Dec,
    /// What a slot costs now, in HYPE; `null` once the round has ended.
    #[serde(default)]
    pub current_gas: Option<Dec>,
    /// What the slot sold at, once it sold.
    #[serde(default)]
    pub end_gas: Option<Dec>,
}

impl GasAuction {
    /// When it ends, Unix seconds.
    #[must_use]
    pub const fn ends_at(&self) -> u64 {
        self.start_time_seconds.saturating_add(self.duration_seconds)
    }

    /// Whether it runs at `now_seconds`: from its start, until its end.
    #[must_use]
    pub const fn is_running(&self, now_seconds: u64) -> bool {
        now_seconds >= self.start_time_seconds && now_seconds < self.ends_at()
    }
}

#[cfg(test)]
#[path = "tests/auction.rs"]
mod tests;
