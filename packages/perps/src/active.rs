//! `activeAssetData`: an account on one perp, with or without a position
//! there: the leverage in force and what an order on each side may take. The
//! venue counts these itself, in every account mode, so they hold where a
//! balance alone would not (a unified account, a position a sell would close).

use hl_core::{Address, ApiError, Transport, info};

use crate::dex::{Dex, request, text};

/// `{"type":"activeAssetData","user":..,"coin":..}`. A HIP-3 perp is named by
/// its full coin, `xyz:TSLA`, which says its dex.
#[must_use]
pub fn active_asset_data_request(user: Address, coin: &str) -> String {
    request("activeAssetData", vec![("user", text(&user.to_hex())), ("coin", text(coin))], Dex::Main)
}

pub async fn active_asset_data(
    transport: &impl Transport,
    user: Address,
    coin: &str,
) -> Result<ActiveAssetData, ApiError> {
    info(transport, active_asset_data_request(user, coin)).await
}

/// The account on one perp: hl-core's, the shape the socket sends.
pub use hl_core::state::ActiveAssetData;
