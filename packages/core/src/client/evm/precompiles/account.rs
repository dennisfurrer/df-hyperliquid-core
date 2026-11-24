//! What an account holds on HyperCore: perp positions, spot balances, vault
//! equity, what it can withdraw, its margin on a dex, and whether it exists.

use crate::client::evm::abi::AbiError;
use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Address;

/// One perp position, as [`position`] and [`position2`] answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Position {
    /// Signed size in the perp's size decimals; negative is short.
    pub szi: i64,
    /// Entry notional, USDC at 1e6.
    pub entry_ntl: u64,
    /// `isolatedRawUsd`: the raw USDC of an isolated position, at 1e6, signed.
    pub isolated_raw_usd: i64,
    /// The position's leverage, as a whole multiple.
    pub leverage: u32,
    /// `isIsolated`: whether the position is isolated rather than cross.
    pub is_isolated: bool,
}

fn decode_position(at: u16, answer: &[u8]) -> Result<Position, AbiError> {
    let tokens = tokens(at, answer)?;
    let mut fields = Fields::of("position", &tokens)?;
    Ok(Position {
        szi: fields.i64()?,
        entry_ntl: fields.u64()?,
        isolated_raw_usd: fields.i64()?,
        leverage: fields.u32()?,
        is_isolated: fields.bool()?,
    })
}

/// `0x800`: one account's position in a perp of the main dex, by a `uint16`
/// index. [`position2`] takes a `uint32`.
#[must_use]
pub fn position(user: Address, perp: u16) -> Read<Position> {
    read(0x800, &[Arg::Address(user), Arg::Uint(u64::from(perp))], |answer| decode_position(0x800, answer))
}

/// `0x813`: [`position`] with a `uint32` perp index, a new address rather than
/// a change to the old one.
#[must_use]
pub fn position2(user: Address, perp: u32) -> Read<Position> {
    read(0x813, &[Arg::Address(user), Arg::Uint(u64::from(perp))], |answer| decode_position(0x813, answer))
}

/// One token's balance on HyperCore's spot side, in its `weiDecimals`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpotBalance {
    /// The whole balance, including what open orders hold.
    pub total: u64,
    /// What open orders hold of it.
    pub hold: u64,
    /// `entryNtl`: entry notional, USDC at 1e6.
    pub entry_ntl: u64,
}

/// `0x801`: a spot balance. `token` is a token index, or an outcome side's
/// asset id, `100000000 + outcome * 10 + side`.
#[must_use]
pub fn spot_balance(user: Address, token: u64) -> Read<SpotBalance> {
    read(0x801, &[Arg::Address(user), Arg::Uint(token)], |answer| {
        let tokens = tokens(0x801, answer)?;
        let mut fields = Fields::of("spotBalance", &tokens)?;
        Ok(SpotBalance { total: fields.u64()?, hold: fields.u64()?, entry_ntl: fields.u64()? })
    })
}

/// An account's equity in one vault, as [`user_vault_equity`] answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserVaultEquity {
    /// USDC at 1e6.
    pub equity: u64,
    /// Unix milliseconds.
    pub locked_until_timestamp: u64,
}

/// `0x802`: an account's equity in a vault, and when it unlocks.
#[must_use]
pub fn user_vault_equity(user: Address, vault: Address) -> Read<UserVaultEquity> {
    read(0x802, &[Arg::Address(user), Arg::Address(vault)], |answer| {
        let tokens = tokens(0x802, answer)?;
        let mut fields = Fields::of("userVaultEquity", &tokens)?;
        Ok(UserVaultEquity { equity: fields.u64()?, locked_until_timestamp: fields.u64()? })
    })
}

/// `0x803`: what the account can withdraw now, USDC at 1e6.
#[must_use]
pub fn withdrawable(user: Address) -> Read<u64> {
    read(0x803, &[Arg::Address(user)], |answer| Fields::of("withdrawable", &tokens(0x803, answer)?)?.u64())
}

/// An account's margin on one perp dex, USDC at 1e6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AccountMarginSummary {
    /// `accountValue`: the account's value on the dex, signed.
    pub account_value: i64,
    /// `marginUsed`: the margin its positions hold.
    pub margin_used: u64,
    /// `ntlPos`: the total notional of its positions.
    pub ntl_pos: u64,
    /// `rawUsd`: its raw USDC balance, signed.
    pub raw_usd: i64,
}

/// `0x80f`: margin on a perp dex, 0 the main one.
#[must_use]
pub fn account_margin_summary(perp_dex_index: u32, user: Address) -> Read<AccountMarginSummary> {
    read(0x80f, &[Arg::Uint(u64::from(perp_dex_index)), Arg::Address(user)], |answer| {
        let tokens = tokens(0x80f, answer)?;
        let mut fields = Fields::of("accountMarginSummary", &tokens)?;
        Ok(AccountMarginSummary {
            account_value: fields.i64()?,
            margin_used: fields.u64()?,
            ntl_pos: fields.u64()?,
            raw_usd: fields.i64()?,
        })
    })
}

/// `0x810`: whether the address exists on HyperCore at all.
#[must_use]
pub fn core_user_exists(user: Address) -> Read<bool> {
    read(0x810, &[Arg::Address(user)], |answer| Fields::of("coreUserExists", &tokens(0x810, answer)?)?.bool())
}

#[cfg(test)]
#[path = "tests/account.rs"]
mod tests;
