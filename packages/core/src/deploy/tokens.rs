//! Spot deploys in progress: an account's unfinished token deployments and the token auction
//! (`spotDeployState`), and the auction for a pair of existing tokens (`spotPairDeployAuctionStatus`).

use crate::client::{ApiError, Transport, info, info_request, user_request};
use crate::deploy::auction::GasAuction;
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `{"type":"spotDeployState","user":..}`.
#[must_use]
pub fn spot_deploy_state_request(user: Address) -> String {
    user_request("spotDeployState", user)
}

pub async fn spot_deploy_state(transport: &impl Transport, user: Address) -> Result<SpotDeployState, ApiError> {
    info(transport, spot_deploy_state_request(user)).await
}

/// `{"type":"spotPairDeployAuctionStatus"}`.
#[must_use]
pub fn spot_pair_deploy_auction_request() -> String {
    info_request("spotPairDeployAuctionStatus", &[])
}

pub async fn spot_pair_deploy_auction(transport: &impl Transport) -> Result<GasAuction, ApiError> {
    info(transport, spot_pair_deploy_auction_request()).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotDeployState {
    /// The account's deployments that have not finished, each until it does.
    #[serde(default)]
    pub states: Vec<DeployingToken>,
    /// The auction a new token's first step pays.
    pub gas_auction: GasAuction,
}

/// One token being deployed. Every field but the token and its spec is read as optional: none has been seen live.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployingToken {
    /// Its index, claimed by the first step.
    pub token: u32,
    pub spec: TokenSpec,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub deployer_trading_fee_share: Option<Dec>,
    /// The pairs registered for it so far, by pair index.
    #[serde(default)]
    pub spots: Vec<u32>,
    /// Set by genesis.
    #[serde(default)]
    pub max_supply: Option<Dec>,
    #[serde(default)]
    pub hyperliquidity_genesis_balance: Option<Dec>,
    #[serde(default)]
    pub total_genesis_balance_wei: Option<Dec>,
    #[serde(default)]
    pub user_genesis_balances: Vec<(Address, Dec)>,
    #[serde(default)]
    pub existing_token_genesis_balances: Vec<(u32, Dec)>,
    #[serde(default)]
    pub blacklist_users: Vec<Address>,
}

/// What the first step registered: fixed for good.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSpec {
    pub name: String,
    pub sz_decimals: u32,
    pub wei_decimals: u32,
}

#[cfg(test)]
#[path = "tests/tokens.rs"]
mod tests;
