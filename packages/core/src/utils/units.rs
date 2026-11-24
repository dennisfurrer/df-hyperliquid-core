//! Amounts an action carries as whole integers: USDC in millionths
//! (`vaultTransfer`, `subAccountTransfer`), HYPE in 1e8 (`cDeposit`,
//! `tokenDelegate`). The integer is what is signed, so it is made here, once.

use crate::utils::dec::{Dec, Round};

/// USDC's decimals on the perp side and on Arbitrum: millionths.
pub const USDC_DECIMALS: u32 = 6;

/// HYPE on HyperCore counts in 1e8: what `wei` means in staking and delegation.
pub const HYPE_DECIMALS: u32 = 8;

/// Why an amount has no integer form to sign.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadUnits {
    /// Zero or negative, or cut to zero units; holds the amount.
    #[error("{0} is not a positive amount")]
    NotPositive(Dec),
    /// [`exact_units`] was given an amount finer than the places allowed.
    #[error("{value} has more than {places} decimal places")]
    TooFine {
        /// The amount given.
        value: Dec,
        /// The most places allowed.
        places: u32,
    },
    /// The count of units does not fit in a `u64`; holds the amount.
    #[error("{0} is too large")]
    TooLarge(Dec),
}

/// `value` as a count of `10^-places`, refused rather than rounded when it is
/// finer than that: what was typed is what is signed.
pub fn exact_units(value: Dec, places: u32) -> Result<u64, BadUnits> {
    if value.places() > places {
        return Err(BadUnits::TooFine { value, places });
    }
    cut_units(value, places)
}

/// `value` cut toward zero to a count of `10^-places`: for a computed figure
/// (a whole balance, say), so it never asks for more than there is.
pub fn cut_units(value: Dec, places: u32) -> Result<u64, BadUnits> {
    if !value.is_positive() {
        return Err(BadUnits::NotPositive(value));
    }
    let units = value.units_at(places, Round::Down).ok_or(BadUnits::TooLarge(value))?;
    match u64::try_from(units) {
        Ok(0) => Err(BadUnits::NotPositive(value)),
        Ok(units) => Ok(units),
        Err(_) => Err(BadUnits::TooLarge(value)),
    }
}

#[cfg(test)]
#[path = "tests/units.rs"]
mod tests;
