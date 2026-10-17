//! `activeAssetData`: an account's leverage on one perp and what an order on each side may take,
//! as the venue counts it in every account mode.

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Json};

/// `{"type":"activeAssetData","user":..,"coin":..}`. A HIP-3 perp is named by
/// its full coin, `xyz:TSLA`, which says its dex.
#[must_use]
pub fn active_asset_data_request(user: Address, coin: &str) -> String {
    info_request("activeAssetData", &[("user", Json::String(user.to_hex())), ("coin", Json::String(coin.to_owned()))])
}

pub async fn active_asset_data(
    transport: &impl Transport,
    user: Address,
    coin: &str,
) -> Result<ActiveAssetData, ApiError> {
    info(transport, active_asset_data_request(user, coin)).await
}

/// The account on one perp, the same shape the socket sends.
pub use crate::account::clearinghouse::ActiveAssetData;
