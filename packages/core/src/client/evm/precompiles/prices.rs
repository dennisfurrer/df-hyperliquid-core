//! Prices: a perp's mark and oracle, a spot pair's price, the best bid and
//! offer, and HyperCore's block height, with the scale each price is read at.

use crate::client::evm::precompiles::fields::{Fields, bare, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Dec;

/// `0x806`: a perp's mark price, in `10^-(6 - szDecimals)` ([`perp_price`]).
#[must_use]
pub fn mark_px(index: u32) -> Read<u64> {
    read(0x806, &[Arg::Uint(u64::from(index))], |answer| bare(0x806, answer))
}

/// `0x807`: a perp's oracle price, scaled as [`mark_px`].
#[must_use]
pub fn oracle_px(index: u32) -> Read<u64> {
    read(0x807, &[Arg::Uint(u64::from(index))], |answer| bare(0x807, answer))
}

/// `0x808`: a spot pair's price, in `10^-(8 - szDecimals)` of its base token
/// ([`spot_price`]). `index` is a pair index, or an outcome side's asset id,
/// `100000000 + outcome * 10 + side`.
#[must_use]
pub fn spot_px(index: u32) -> Read<u64> {
    read(0x808, &[Arg::Uint(u64::from(index))], |answer| bare(0x808, answer))
}

/// `0x809`: HyperCore's block height. The one precompile with no arguments.
#[must_use]
pub fn l1_block_number() -> Read<u64> {
    read(0x809, &[], |answer| bare(0x809, answer))
}

/// The best bid and offer, scaled as the market's prices are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bbo {
    /// The best bid, unscaled; read it with [`perp_price`] or [`spot_price`].
    pub bid: u64,
    /// The best ask, unscaled; read it with [`perp_price`] or [`spot_price`].
    pub ask: u64,
}

/// `0x80e`: the best bid and offer of a perp (its index) or a spot pair
/// (10000 plus its index).
#[must_use]
pub fn bbo(asset: u32) -> Read<Bbo> {
    read(0x80e, &[Arg::Uint(u64::from(asset))], |answer| {
        let tokens = tokens(0x80e, answer)?;
        let mut fields = Fields::of("bbo", &tokens)?;
        Ok(Bbo { bid: fields.u64()?, ask: fields.u64()? })
    })
}

/// A perp price from [`mark_px`], [`oracle_px`] or [`bbo`]: `raw / 10^(6 - szDecimals)`.
#[must_use]
pub fn perp_price(raw: u64, sz_decimals: u8) -> Option<Dec> {
    Dec::new(i128::from(raw), 6u32.checked_sub(u32::from(sz_decimals))?)
}

/// A spot price from [`spot_px`] or [`bbo`]: `raw / 10^(8 - szDecimals)` of the base token.
#[must_use]
pub fn spot_price(raw: u64, base_sz_decimals: u8) -> Option<Dec> {
    Dec::new(i128::from(raw), 8u32.checked_sub(u32::from(base_sz_decimals))?)
}

#[cfg(test)]
#[path = "tests/prices.rs"]
mod tests;
