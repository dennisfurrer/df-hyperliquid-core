//! Amounts an action carries as whole integers: USDC in millionths
//! (`vaultTransfer`, `subAccountTransfer`), HYPE in 1e8 (`cDeposit`,
//! `tokenDelegate`). The integer is what is signed, so it is made here, once.

use crate::dec::{Dec, Round};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadUnits {
    #[error("{0} is not a positive amount")]
    NotPositive(Dec),
    #[error("{value} has more than {places} decimal places")]
    TooFine { value: Dec, places: u32 },
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
mod tests {
    use super::*;

    fn dec(text: &str) -> Dec {
        Dec::parse(text).unwrap()
    }

    #[test]
    fn an_exact_amount_is_counted_and_a_finer_one_refused() {
        assert_eq!(exact_units(dec("10"), 6), Ok(10_000_000));
        assert_eq!(exact_units(dec("2.5"), 6), Ok(2_500_000));
        assert_eq!(exact_units(dec("100"), 8), Ok(10_000_000_000));
        assert_eq!(exact_units(dec("0.00000001"), 8), Ok(1));
        assert_eq!(exact_units(dec("1.0000001"), 6), Err(BadUnits::TooFine { value: dec("1.0000001"), places: 6 }));
        assert_eq!(exact_units(Dec::ZERO, 6), Err(BadUnits::NotPositive(Dec::ZERO)));
        assert_eq!(exact_units(dec("-1"), 6), Err(BadUnits::NotPositive(dec("-1"))));
        assert_eq!(exact_units(dec("18446744073709.551616"), 6), Err(BadUnits::TooLarge(dec("18446744073709.551616"))));
    }

    #[test]
    fn a_computed_amount_is_cut_toward_zero() {
        assert_eq!(cut_units(dec("604931.6453119999"), 6), Ok(604_931_645_311));
        assert_eq!(
            cut_units(dec("0.0000009"), 6),
            Err(BadUnits::NotPositive(dec("0.0000009"))),
            "dust cuts to nothing"
        );
    }
}
