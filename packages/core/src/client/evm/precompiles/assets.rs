//! Market info: a perp's asset info, a spot pair, a token and its EVM contract,
//! and a token's supply.

use crate::client::evm::abi::AbiError;
use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerpAssetInfo {
    pub coin: String,
    pub margin_table_id: u32,
    pub sz_decimals: u8,
    pub max_leverage: u8,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotInfo {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    pub name: String,
    /// The spot pairs it trades in.
    pub spots: Vec<u64>,
    pub deployer_trading_fee_share: u64,
    pub deployer: Address,
    /// The linked ERC-20 on HyperEVM; zero when it has none.
    pub evm_contract: Address,
    pub sz_decimals: u8,
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
pub struct UserBalance {
    pub user: Address,
    pub balance: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSupply {
    pub max_supply: u64,
    pub total_supply: u64,
    pub circulating_supply: u64,
    pub future_emissions: u64,
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
