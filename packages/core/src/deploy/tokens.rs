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

/// The account's unfinished spot token deployments and the current token auction.
pub async fn spot_deploy_state(transport: &impl Transport, user: Address) -> Result<SpotDeployState, ApiError> {
    info(transport, spot_deploy_state_request(user)).await
}

/// `{"type":"spotPairDeployAuctionStatus"}`.
#[must_use]
pub fn spot_pair_deploy_auction_request() -> String {
    info_request("spotPairDeployAuctionStatus", &[])
}

/// The current round of the spot pair deploy auction.
pub async fn spot_pair_deploy_auction(transport: &impl Transport) -> Result<GasAuction, ApiError> {
    info(transport, spot_pair_deploy_auction_request()).await
}

/// What `spotDeployState` answers for one account.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SpotDeployState {
    /// The account's deployments that have not finished, each until it does.
    #[serde(default)]
    pub states: Vec<DeployingToken>,
    /// The auction a new token's first step pays.
    pub gas_auction: GasAuction,
}

/// One token being deployed. Every field but the token and its spec is read as optional.
///
/// The balances are whole tokens as the venue renders them through a 64-bit float, so a large
/// one is rounded: `184467440737.0955200195` for a total of `18446744073709551615` wei. Only
/// [`total_genesis_balance_wei`](DeployingToken::total_genesis_balance_wei) is exact. Confirmed
/// against testnet deploys in progress and the `userGenesis` actions behind them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DeployingToken {
    /// Its index, claimed by the first step.
    pub token: u32,
    /// The name, size decimals and wei decimals registered.
    pub spec: TokenSpec,
    /// The full name (`fullName`), when one was registered.
    #[serde(default)]
    pub full_name: Option<String>,
    /// The deployer's share of trading fees (`deployerTradingFeeShare`), as a fraction.
    #[serde(default)]
    pub deployer_trading_fee_share: Option<Dec>,
    /// The pairs registered for it so far, by pair index.
    #[serde(default)]
    pub spots: Vec<u32>,
    /// Set by genesis, in whole tokens; `None` before.
    #[serde(default)]
    pub max_supply: Option<Dec>,
    /// The supply kept for hyperliquidity at genesis (`hyperliquidityGenesisBalance`), in whole tokens.
    #[serde(default)]
    pub hyperliquidity_genesis_balance: Option<Dec>,
    /// The total given out at genesis, in the token's wei (`totalGenesisBalanceWei`): exact.
    #[serde(default)]
    pub total_genesis_balance_wei: Option<Dec>,
    /// What each address was given by the user genesis steps (`userGenesisBalances`), in whole
    /// tokens. Emptied once genesis is done.
    #[serde(default)]
    pub user_genesis_balances: Vec<(Address, Dec)>,
    /// What was shared among the holders of each existing token (`existingTokenGenesisBalances`), by
    /// token index, in whole tokens.
    #[serde(default)]
    pub existing_token_genesis_balances: Vec<(u32, Dec)>,
    /// Addresses blacklisted from holding the token (`blacklistUsers`).
    #[serde(default)]
    pub blacklist_users: Vec<Address>,
}

/// What the first step registered: fixed for good.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSpec {
    /// The token's name.
    pub name: String,
    /// Size decimals (`szDecimals`): how many decimal places a spot order size may have.
    pub sz_decimals: u32,
    /// Wei decimals (`weiDecimals`): how many decimal places a balance is kept to.
    pub wei_decimals: u32,
}

#[cfg(test)]
#[path = "tests/tokens.rs"]
mod tests;
