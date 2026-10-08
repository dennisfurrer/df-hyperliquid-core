//! A few reads send a number as a bare JSON float rather than a decimal
//! string: `vaultDetails` writes `"apr":0.037456007367970744`. serde_json
//! would put that through an f64 and keep whatever digits came back out; this
//! reads the text the venue wrote instead.

use std::fmt;

use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::dec::Dec;

/// A number exactly as the venue wrote it, float or string. No f64 holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Figure(String);

impl Figure {
    /// The digits as written: `0.037456007367970744`, `1e-7`, `0.0`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The value, an exponent applied exactly. Past eighteen places it is the
    /// nearest at eighteen; `None` past what a [`Dec`] counts.
    #[must_use]
    pub fn to_dec(&self) -> Option<Dec> {
        let (mantissa, exponent) = match self.0.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<i32>().ok()?),
            None => (self.0.as_str(), 0),
        };
        let mut value = Dec::parse(mantissa).ok()?;
        let mut shift = exponent.unsigned_abs();
        while shift > 0 {
            let step = shift.min(18);
            let factor = match exponent < 0 {
                true => Dec::new(1, step)?,
                false => Dec::new(10i128.checked_pow(step)?, 0)?,
            };
            value = value.checked_mul(factor)?;
            shift -= step;
        }
        Some(value)
    }

    fn read(text: &str) -> Option<Figure> {
        let plain = |text: &str| {
            !text.is_empty()
                && text.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E'))
        };
        match text.strip_prefix('"') {
            Some(_) => serde_json::from_str::<String>(text).ok().filter(|inner| plain(inner)).map(Figure),
            None => plain(text).then(|| Figure(text.to_owned())),
        }
    }
}

impl fmt::Display for Figure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// From the raw JSON text, so it reads only through `serde_json::from_str`
/// (which is how `info` reads), not through a `serde_json::Value`.
impl<'de> Deserialize<'de> for Figure {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        Figure::read(raw.get().trim()).ok_or_else(|| de::Error::custom(format!("`{}` is not a number", raw.get())))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct Vault {
        apr: Figure,
        #[serde(default)]
        commission: Option<Figure>,
    }

    /// Live, mainnet HLP, 2026-10-06: eighteen places, which an f64 cannot
    /// carry and a `Dec` can.
    #[test]
    fn a_float_keeps_every_digit_it_was_written_with() {
        let vault: Vault = serde_json::from_str(r#"{"apr":0.037569521048262375,"commission":0.0}"#).unwrap();
        assert_eq!(vault.apr.as_str(), "0.037569521048262375");
        assert_eq!(vault.apr.to_dec(), Dec::new(37_569_521_048_262_375, 18));
        assert_eq!(vault.commission.and_then(|figure| figure.to_dec()), Some(Dec::ZERO));
        let none: Vault = serde_json::from_str(r#"{"apr":"0.05","commission":null}"#).unwrap();
        assert_eq!(none.apr.to_dec(), Dec::new(5, 2), "a decimal string reads too");
        assert_eq!(none.commission, None);
    }

    #[test]
    fn an_exponent_is_applied_exactly() {
        let read = |text: &str| serde_json::from_str::<Figure>(text).unwrap().to_dec();
        assert_eq!(read("1e-7"), Dec::new(1, 7));
        assert_eq!(read("2.5E3"), Some(Dec::from_int(2500)));
        assert_eq!(read("-4.2e-1"), Dec::new(-42, 2));
        assert_eq!(read("1e-30"), Some(Dec::ZERO), "past eighteen places is the nearest at eighteen");
        assert_eq!(read("41085245.621082"), Dec::new(41_085_245_621_082, 6));
    }

    #[test]
    fn anything_but_a_number_is_refused() {
        for bad in ["true", "\"abc\"", "{}", "[1]", "null", "\"\""] {
            assert!(serde_json::from_str::<Figure>(bad).is_err(), "{bad}");
        }
    }
}
