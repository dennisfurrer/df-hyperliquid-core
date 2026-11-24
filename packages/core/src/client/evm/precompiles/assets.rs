//! Market info: a perp's asset info, a spot pair, a token and its EVM contract,
//! and a token's supply.

use crate::client::evm::abi::AbiError;
use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Address;

/// A perp's asset info, as [`perp_asset_info`] answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PerpAssetInfo {
    /// The perp's name, such as `BTC`.
    pub coin: String,
    /// `marginTableId`: the margin table that sets its leverage tiers.
    pub margin_table_id: u32,
    /// `szDecimals`: the decimals its sizes are counted in.
    pub sz_decimals: u8,
    /// `maxLeverage`: the most leverage it allows, as a whole multiple.
    pub max_leverage: u8,
    /// `onlyIsolated`: whether it trades only with isolated margin.
    pub only_isolated: bool,
}

/// `0x80a`: a perp's name, size decimals and leverage cap.
#[must_use]
pub fn perp_asset_info(perp: u32) -> Read<PerpAssetInfo> {
    read(0x80a, &[Arg::Uint(u64::from(perp))], |answer| {
        let tokens = tokens(0x80a, answer)?;
        let mut fields = Fields::of("perpAssetInfo", &tokens)?;
        Ok(PerpAssetInfo {
            coin: fields.string()?,
            margin_table_id: fields.u32()?,
            sz_decimals: fields.u8()?,
            max_leverage: fields.u8()?,
            only_isolated: fields.bool()?,
        })
    })
}

/// A spot pair, as [`spot_info`] answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpotInfo {
    /// The pair's name, such as `PURR/USDC` or `@107`.
    pub name: String,
    /// The base token's index, then the quote's.
    pub tokens: [u64; 2],
}

/// `0x80b`: a spot pair and the two tokens it is between.
#[must_use]
pub fn spot_info(spot: u32) -> Read<SpotInfo> {
    read(0x80b, &[Arg::Uint(u64::from(spot))], |answer| {
        let tokens = tokens(0x80b, answer)?;
        let mut fields = Fields::of("spotInfo", &tokens)?;
        let name = fields.string()?;
        let pair: [u64; 2] =
            fields.u64s()?.try_into().map_err(|_| AbiError::Type("spotInfo: not two tokens".to_owned()))?;
        Ok(SpotInfo { name, tokens: pair })
    })
}

/// A token, as [`token_info`] answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TokenInfo {
    /// The token's name, such as `HYPE`.
    pub name: String,
    /// The spot pairs it trades in.
    pub spots: Vec<u64>,
    /// `deployerTradingFeeShare`: the deployer's share of trading fees.
    pub deployer_trading_fee_share: u64,
    /// The address that deployed the token; zero for none.
    pub deployer: Address,
    /// The linked ERC-20 on HyperEVM; zero when it has none.
    pub evm_contract: Address,
    /// `szDecimals`: the decimals its sizes are counted in.
    pub sz_decimals: u8,
    /// `weiDecimals`: the decimals its balances are counted in on HyperCore.
    pub wei_decimals: u8,
    /// How many more decimals the ERC-20 counts in than HyperCore does.
    pub evm_extra_wei_decimals: i8,
}

/// `0x80c`: everything about a token, its EVM contract included.
#[must_use]
pub fn token_info(token: u32) -> Read<TokenInfo> {
    read(0x80c, &[Arg::Uint(u64::from(token))], |answer| {
        let tokens = tokens(0x80c, answer)?;
        let mut fields = Fields::of("tokenInfo", &tokens)?;
        Ok(TokenInfo {
            name: fields.string()?,
            spots: fields.u64s()?,
            deployer_trading_fee_share: fields.u64()?,
            deployer: fields.address()?,
            evm_contract: fields.address()?,
            sz_decimals: fields.u8()?,
            wei_decimals: fields.u8()?,
            evm_extra_wei_decimals: fields.i8()?,
        })
    })
}

/// An account holding non-circulating supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserBalance {
    /// The holding account.
    pub user: Address,
    /// What it holds, in the token's `weiDecimals`.
    pub balance: u64,
}

/// A token's supply, as [`token_supply`] answers it, in the token's `weiDecimals`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TokenSupply {
    /// `maxSupply`: the most there can ever be.
    pub max_supply: u64,
    /// `totalSupply`: what exists now.
    pub total_supply: u64,
    /// `circulatingSupply`: what exists now, less the non-circulating balances.
    pub circulating_supply: u64,
    /// `futureEmissions`: what is still to be emitted.
    pub future_emissions: u64,
    /// `nonCirculatingUserBalances`: the accounts whose balances do not count as circulating.
    pub non_circulating_user_balances: Vec<UserBalance>,
}

/// `0x80d`: a token's supply, and who holds the part that does not circulate.
#[must_use]
pub fn token_supply(token: u32) -> Read<TokenSupply> {
    read(0x80d, &[Arg::Uint(u64::from(token))], |answer| {
        let tokens = tokens(0x80d, answer)?;
        let mut fields = Fields::of("tokenSupply", &tokens)?;
        let (max_supply, total_supply, circulating_supply, future_emissions) =
            (fields.u64()?, fields.u64()?, fields.u64()?, fields.u64()?);
        let holders = fields
            .items()?
            .iter()
            .map(|item| {
                let mut holder = Fields::of("tokenSupply", std::slice::from_ref(item))?;
                Ok(UserBalance { user: holder.address()?, balance: holder.u64()? })
            })
            .collect::<Result<_, AbiError>>()?;
        Ok(TokenSupply {
            max_supply,
            total_supply,
            circulating_supply,
            future_emissions,
            non_circulating_user_balances: holders,
        })
    })
}

#[cfg(test)]
#[path = "tests/assets.rs"]
mod tests;
