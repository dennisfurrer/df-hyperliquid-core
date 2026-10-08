//! `perpDeployAuctionStatus`: the Dutch auction every dex shares for a perp
//! past a dex's first three, 31 hours long, its price in HYPE and never
//! refunded. A reserve deployment skips it.

use hl_core::{ApiError, Dec, Transport, info};
use serde::Deserialize;

use crate::dex::request;

/// `{"type":"perpDeployAuctionStatus"}`.
#[must_use]
pub fn perp_deploy_auction_request() -> String {
    request("perpDeployAuctionStatus", None)
}

pub async fn perp_deploy_auction(transport: &impl Transport) -> Result<DeployAuction, ApiError> {
    info(transport, perp_deploy_auction_request()).await
}

/// One round of the auction. Seconds, not milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployAuction {
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

impl DeployAuction {
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
