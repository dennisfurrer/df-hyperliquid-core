//! Amounts the venue is asked to move: plain decimals, at most eight places, never zero, in the
//! normal form that is hashed (`"12.50"` is `"12.5"`, `"007"` is `"7"`).

use std::fmt;

use crate::utils::{Dec, Round};

/// The most places an amount the venue takes may have.
pub const MAX_DECIMALS: u32 = 8;

/// Why an amount is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadAmount {
    /// Nothing, or only spaces, was given.
    #[error("no amount given")]
    Empty,
    /// Not digits with at most one point and digits before it: `1,000`, `1e5`, `.5`, `+5`.
    #[error("`{0}` is not a plain decimal amount")]
    NotDecimal(String),
    /// More than [`MAX_DECIMALS`] places.
    #[error("`{0}` has more than {MAX_DECIMALS} decimal places")]
    TooFine(String),
    /// Zero, or a computed figure that cuts to zero.
    #[error("the amount is zero")]
    Zero,
    /// Below zero.
    #[error("`{0}` is below zero")]
    Negative(String),
    /// Too large for a [`Dec`].
    #[error("`{0}` is too large")]
    TooLarge(String),
}

/// A validated amount: its wire string and the same value as a [`Dec`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amount {
    wire: String,
    value: Dec,
}

impl Amount {
    /// What a person typed, checked by [`clean_amount`].
    pub fn parse(text: &str) -> Result<Amount, BadAmount> {
        let wire = clean_amount(text)?;
        let value = Dec::parse(&wire).map_err(|_| BadAmount::TooLarge(wire.clone()))?;
        Ok(Amount { wire, value })
    }

    /// A computed figure cut toward zero to `places` (at most eight), so it never asks for more
    /// than is there.
    pub fn cut(value: Dec, places: u32) -> Result<Amount, BadAmount> {
        if value.is_negative() {
            return Err(BadAmount::Negative(value.to_wire()));
        }
        let value = value.round(places.min(MAX_DECIMALS), Round::Down);
        if value.is_zero() {
            return Err(BadAmount::Zero);
        }
        Ok(Amount { wire: value.to_wire(), value })
    }

    /// The string that is posted and hashed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.wire
    }

    /// The amount as a [`Dec`].
    #[must_use]
    pub const fn value(&self) -> Dec {
        self.value
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.wire)
    }
}

/// Checks a typed amount: digits, at most one point with digits before it, at most eight places,
/// not zero. Returns the normal form; a place finer than eight is refused, never rounded.
pub fn clean_amount(text: &str) -> Result<String, BadAmount> {
    let text = text.trim();
    if text.is_empty() {
        return Err(BadAmount::Empty);
    }
    if let Some(rest) = text.strip_prefix('-')
        && !rest.is_empty()
        && rest.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    {
        return Err(BadAmount::Negative(text.to_owned()));
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BadAmount::NotDecimal(text.to_owned()));
    }
    if fraction.trim_end_matches('0').len() > MAX_DECIMALS as usize {
        return Err(BadAmount::TooFine(text.to_owned()));
    }
    let value = Dec::parse(text).map_err(|_| BadAmount::TooLarge(text.to_owned()))?;
    if value.is_zero() {
        return Err(BadAmount::Zero);
    }
    Ok(value.to_wire())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> Dec {
        Dec::parse(text).unwrap()
    }

    #[test]
    fn an_amount_is_checked_before_anything_is_signed() {
        assert_eq!(clean_amount(" 100.5 ").unwrap(), "100.5");
        assert_eq!(clean_amount("0.00000001").unwrap(), "0.00000001");
        for (bad, why) in [
            ("", BadAmount::Empty),
            ("0", BadAmount::Zero),
            ("0.00", BadAmount::Zero),
            ("-5", BadAmount::Negative("-5".to_owned())),
            ("1,000", BadAmount::NotDecimal("1,000".to_owned())),
            ("1e5", BadAmount::NotDecimal("1e5".to_owned())),
            ("abc", BadAmount::NotDecimal("abc".to_owned())),
            ("1.000000001", BadAmount::TooFine("1.000000001".to_owned())),
            (".", BadAmount::NotDecimal(".".to_owned())),
            (".5", BadAmount::NotDecimal(".5".to_owned())),
            ("+5", BadAmount::NotDecimal("+5".to_owned())),
            ("1.2.3", BadAmount::NotDecimal("1.2.3".to_owned())),
        ] {
            assert_eq!(clean_amount(bad), Err(why), "`{bad}` should not be signable");
        }
    }

    /// The SDK's `formatDecimalString`: the hashed string is the normal one.
    #[test]
    fn an_amount_comes_back_in_the_form_the_sdk_hashes() {
        for (typed, wire) in [
            ("100", "100"),
            ("100.0", "100"),
            ("12.50", "12.5"),
            ("000.5", "0.5"),
            ("007", "7"),
            ("1.", "1"),
            ("1.000000000", "1"),
            ("2550.12345678", "2550.12345678"),
        ] {
            assert_eq!(clean_amount(typed).unwrap(), wire, "{typed}");
            assert_eq!(Amount::parse(typed).unwrap().as_str(), wire, "{typed}");
        }
        assert_eq!(Amount::parse("12.50").unwrap().value(), dec("12.5"));
    }

    #[test]
    fn a_computed_figure_is_cut_toward_zero_never_rounded_up() {
        assert_eq!(Amount::cut(dec("41085515.0539499968"), 6).unwrap().as_str(), "41085515.053949");
        assert_eq!(Amount::cut(dec("0.123456789"), 12).unwrap().as_str(), "0.12345678", "eight places at most");
        assert_eq!(Amount::cut(dec("0.0000009"), 6), Err(BadAmount::Zero), "dust cuts to nothing");
        assert_eq!(Amount::cut(dec("-1"), 6), Err(BadAmount::Negative("-1".to_owned())));
    }
}
