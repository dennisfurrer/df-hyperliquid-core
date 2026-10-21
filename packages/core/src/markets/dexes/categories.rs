//! `perpCategories`: what kind of market each HIP-3 perp is, as its deployer filed it. Perps on the
//! main dex have none.

use crate::client::{ApiError, Transport, info, info_request};

/// `{"type":"perpCategories"}`.
#[must_use]
pub fn perp_categories_request() -> String {
    info_request("perpCategories", &[])
}

/// Each HIP-3 perp's category by its coin: `("xyz:TSLA", "stocks")`. The words are the deployers'
/// own and kept as sent, so one dex's `FX` is another's `fx`, and one files a stock as `stock`.
pub async fn perp_categories(transport: &impl Transport) -> Result<Vec<(String, String)>, ApiError> {
    info(transport, perp_categories_request()).await
}
