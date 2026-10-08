//! Spot deploys in progress: `spotDeployState` (an account's unfinished
//! token deployments and the token auction) and `spotPairDeployAuctionStatus`
//! (the auction for a pair between two tokens that both exist). Each auction
//! is Dutch, 31 hours long; its price is HYPE and is never refunded.
//!
//! The shapes are `@nktkas/hyperliquid` 0.32.1's. A deployment in progress
//! (`states`) has not been seen live here, so all of its fields but the
//! token and its spec are read as optional.

use hl_core::{Address, ApiError, Dec, Transport, info, info_request, user_request};
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

/// A Dutch auction for a deploy slot. Seconds, not milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAuction {
    pub start_time_seconds: u64,
    pub duration_seconds: u64,
    /// HYPE.
    pub start_gas: Dec,
    /// What a slot costs now, in HYPE; `null` once the auction has ended.
    #[serde(default)]
    pub current_gas: Option<Dec>,
    /// What the slot sold at, once it sold.
    #[serde(default)]
    pub end_gas: Option<Dec>,
}

impl GasAuction {
    /// When it ends, Unix seconds.
    #[must_use]
    pub const fn ends_at(&self) -> u64 {
        self.start_time_seconds.saturating_add(self.duration_seconds)
    }

    /// Whether it runs at `now_seconds`: from its start, until its end.
    #[must_use]
    pub const fn is_running(&self, now_seconds: u64) -> bool {
        now_seconds >= self.start_time_seconds && now_seconds < self.ends_at()
    }
}

/// One token being deployed.
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
