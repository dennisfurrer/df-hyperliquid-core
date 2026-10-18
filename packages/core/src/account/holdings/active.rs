//! `activeAssetData`: an account on one perp, with or without a position there: the leverage in
//! force and what an order on each side may take, as the venue counts it in every account mode.
//! The same shape the socket sends.

use serde::Deserialize;

use crate::account::holdings::perps::Leverage;
use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::Side;
use crate::utils::{Address, Dec, Json};

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

/// The account on one perp: the leverage in force and what an order on each side may take.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAssetData {
    pub user: Address,
    pub coin: String,
    /// What an order here would be margined at.
    pub leverage: Leverage,
    /// The largest size an order may be, buy then sell, at the leverage over the mark.
    pub max_trade_szs: [Dec; 2],
    /// What an order may draw on, in the collateral, buy then sell.
    pub available_to_trade: [Dec; 2],
    pub mark_px: Dec,
}

impl ActiveAssetData {
    /// The largest size an order on `side` may be.
    #[must_use]
    pub const fn max_size(&self, side: Side) -> Dec {
        self.max_trade_szs[at(side)]
    }

    /// What an order on `side` may draw on.
    #[must_use]
    pub const fn available(&self, side: Side) -> Dec {
        self.available_to_trade[at(side)]
    }
}

/// Buy then sell, as the venue lists both sides.
const fn at(side: Side) -> usize {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
    }
}

#[cfg(test)]
#[path = "tests/active.rs"]
mod tests;
