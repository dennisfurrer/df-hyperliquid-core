//! A holding in one outcome side: whole contracts that may be short, as fills add them up.

use std::{fmt, str::FromStr};

use crate::markets::outcomes::px::qty::{BadQty, Qty};
use crate::utils::Dec;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Why a figure is not a holding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadSignedQty {
    /// Not whole contracts.
    #[error(transparent)]
    Size(#[from] BadQty),
    /// More contracts than an `i64` counts; holds the text.
    #[error("holding `{0}` is more contracts than this build can count")]
    TooLarge(String),
}

/// Whole contracts that may be short: what fills say is held in a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SignedQty(i64);

impl SignedQty {
    /// No holding.
    pub const ZERO: SignedQty = SignedQty(0);

    /// `contracts` whole contracts, negative for short.
    #[must_use]
    pub const fn new(contracts: i64) -> SignedQty {
        SignedQty(contracts)
    }

    /// The number of contracts, negative for short.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    /// Whether nothing is held.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Whether the holding is short.
    #[must_use]
    pub const fn is_short(self) -> bool {
        self.0 < 0
    }

    /// The number of contracts held, long or short.
    #[must_use]
    pub const fn abs(self) -> Qty {
        Qty::new(self.0.unsigned_abs())
    }

    /// Saturates at what an `i64` counts.
    #[must_use]
    pub const fn long(size: Qty) -> SignedQty {
        match size.get() > i64::MAX as u64 {
            true => SignedQty(i64::MAX),
            false => SignedQty(size.get() as i64),
        }
    }

    /// A short holding of `size`; saturates at what an `i64` counts.
    #[must_use]
    pub const fn short(size: Qty) -> SignedQty {
        match size.get() > i64::MAX as u64 {
            true => SignedQty(i64::MIN),
            false => SignedQty(-(size.get() as i64)),
        }
    }

    /// The sum of the two holdings, stopping at what an `i64` counts.
    #[must_use]
    pub const fn saturating_add(self, other: SignedQty) -> SignedQty {
        SignedQty(self.0.saturating_add(other.0))
    }

    /// Reads a holding, taking the `-539.0` the venue prints.
    pub fn parse(text: &str) -> Result<SignedQty, BadSignedQty> {
        let raw = text.trim();
        let (negative, body) = match raw.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, raw),
        };
        let size = Qty::parse(body).map_err(|error| match error {
            BadQty::NotAnInteger(_) => BadQty::NotAnInteger(raw.to_owned()),
            BadQty::Fractional(_) => BadQty::Fractional(raw.to_owned()),
        })?;
        let contracts = i64::try_from(size.get()).map_err(|_| BadSignedQty::TooLarge(raw.to_owned()))?;
        Ok(SignedQty(if negative { -contracts } else { contracts }))
    }

    /// A decimal that is a whole number of contracts, either side of zero.
    pub fn from_dec(value: Dec) -> Result<SignedQty, BadSignedQty> {
        let size = Qty::from_dec(value.abs())?;
        let contracts = i64::try_from(size.get()).map_err(|_| BadSignedQty::TooLarge(value.to_string()))?;
        Ok(SignedQty(if value.is_negative() { -contracts } else { contracts }))
    }

    /// The holding as the wire writes it: digits with a leading `-` when short.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for SignedQty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SignedQty {
    type Err = BadSignedQty;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        SignedQty::parse(text)
    }
}

/// As the wire writes it: a string, the way [`SignedQty::parse`] reads it.
impl Serialize for SignedQty {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.to_wire())
    }
}

impl<'de> Deserialize<'de> for SignedQty {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        SignedQty::parse(&text).map_err(de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/signed_qty.rs"]
mod tests;
