//! `tokenDetails`: one token's supply, prices and deployment, asked by its token id (`0x` and 32
//! hex digits, as `spotMeta` writes it).

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Dec, Json};
use serde::Deserialize;

/// `{"type":"tokenDetails","tokenId":..}`.
#[must_use]
pub fn token_details_request(token_id: &str) -> String {
    info_request("tokenDetails", &[("tokenId", Json::String(token_id.to_owned()))])
}

/// HYPE's answer carries every genesis balance, near 100,000 of them: megabytes.
pub async fn token_details(transport: &impl Transport, token_id: &str) -> Result<TokenDetails, ApiError> {
    info(transport, token_details_request(token_id)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDetails {
    pub name: String,
    pub max_supply: Dec,
    pub total_supply: Dec,
    pub circulating_supply: Dec,
    pub sz_decimals: u32,
    pub wei_decimals: u32,
    /// Prices on its pair with USDC; left out without one.
    #[serde(default)]
    pub mid_px: Option<Dec>,
    #[serde(default)]
    pub mark_px: Option<Dec>,
    #[serde(default)]
    pub prev_day_px: Option<Dec>,
    /// `null` once a deployment has finished with none left to read.
    #[serde(default)]
    pub genesis: Option<Genesis>,
    /// `null` for a token the venue itself deployed (HYPE, USDC).
    #[serde(default)]
    pub deployer: Option<Address>,
    /// What the deployer paid at the auction, in HYPE.
    #[serde(default)]
    pub deploy_gas: Option<Dec>,
    /// As the venue writes it: `2024-11-29T06:45:52.532`, UTC, no zone.
    #[serde(default)]
    pub deploy_time: Option<String>,
    /// The USDC the deployer seeded its order book's bids with.
    pub seeded_usdc: Dec,
    /// Balances not counted as circulating.
    #[serde(default)]
    pub non_circulating_user_balances: Vec<(Address, Dec)>,
    pub future_emissions: Dec,
}

/// What a token was deployed with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Genesis {
    pub user_balances: Vec<(Address, Dec)>,
    /// Shares handed to the holders of other tokens, by token index.
    #[serde(default)]
    pub existing_token_balances: Vec<(u32, Dec)>,
    #[serde(default)]
    pub blacklist_users: Vec<Address>,
}
