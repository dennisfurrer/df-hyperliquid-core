//! `perpDeployAuctionStatus`: the auction every dex shares for a perp past a dex's first three. A reserve
//! deployment skips it.

use crate::client::{ApiError, Transport, info};
use crate::markets::dexes::dex::request;

/// `{"type":"perpDeployAuctionStatus"}`.
#[must_use]
pub fn perp_deploy_auction_request() -> String {
    request("perpDeployAuctionStatus", None)
}

pub async fn perp_deploy_auction(transport: &impl Transport) -> Result<DeployAuction, ApiError> {
    info(transport, perp_deploy_auction_request()).await
}

/// One round of the perp deploy auction, the [`GasAuction`](crate::deploy::auction::GasAuction) every deploy
/// auction answers.
pub use crate::deploy::auction::GasAuction as DeployAuction;
