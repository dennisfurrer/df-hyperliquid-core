//! `activeAssetData`: an account on one perp, with or without a position
//! there: the leverage in force and what an order on each side may take. The
//! venue counts these itself, in every account mode, so they hold where a
//! balance alone would not (a unified account, a position a sell would close).

use hl_core::{Address, ApiError, Dec, Transport, info};
use hl_trade::Side;
use serde::Deserialize;

use crate::{
    dex::{Dex, request, text},
    state::Leverage,
};

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAssetData {
    pub user: Address,
    pub coin: String,
    /// What an order here would be margined at.
    pub leverage: Leverage,
    /// The largest size an order may be, buy then sell, at the leverage over
    /// the mark.
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

const fn at(side: Side) -> usize {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
    }
}
