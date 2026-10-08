//! `perpDeployAuctionStatus`: the Dutch auction every dex shares for a perp
//! past a dex's first three, 31 hours long, its price in HYPE and never
//! refunded. A reserve deployment skips it.

use hl_core::{ApiError, Transport, info};

use crate::dex::request;

/// `{"type":"perpDeployAuctionStatus"}`.
#[must_use]
pub fn perp_deploy_auction_request() -> String {
    request("perpDeployAuctionStatus", None)
}

pub async fn perp_deploy_auction(transport: &impl Transport) -> Result<DeployAuction, ApiError> {
    info(transport, perp_deploy_auction_request()).await
}

/// One round of the auction: hl-core's [`GasAuction`](hl_core::GasAuction),
/// the shape every deploy auction answers, by the name this crate gives it.
pub use hl_core::GasAuction as DeployAuction;
