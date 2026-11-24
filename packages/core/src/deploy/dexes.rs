//! `perpDeployAuctionStatus`: the auction every dex shares for a perp past a dex's first three. A reserve
//! deployment skips it.

use crate::client::{ApiError, Transport, info, info_request};
use crate::deploy::auction::GasAuction;

/// `{"type":"perpDeployAuctionStatus"}`.
#[must_use]
pub fn perp_deploy_auction_request() -> String {
    info_request("perpDeployAuctionStatus", &[])
}

/// The current round of the shared HIP-3 perp deploy auction.
pub async fn perp_deploy_auction(transport: &impl Transport) -> Result<GasAuction, ApiError> {
    info(transport, perp_deploy_auction_request()).await
}
