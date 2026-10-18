//! The two decimals an outcome action carries, each in its range and sent in normal form: a
//! deployer's [`FeeScale`] and a settlement's [`Fraction`].

use std::{fmt, str::FromStr};

use crate::deploy::outcomes::actions::error::DeployError;
use crate::utils::Dec;

/// `text` as a decimal from 0 to `most`, written with no sign and no
/// exponent. It goes on the wire in normal form: `1.0` as `1`.
fn bounded(text: &str, most: Dec) -> Option<Dec> {
    let text = text.trim();
    if text.starts_with(['+', '-']) {
        return None;
    }
    let value = Dec::parse(text).ok()?;
    (!value.is_negative() && value <= most).then_some(value)
}

/// What a deployer takes of the fees on its outcomes, from 0 to 10.
/// `outcomeMeta` reads a scale back as `1.0`; it is sent as `1`. Only a
/// standalone outcome and a question carry one: a named outcome added later
/// takes its question's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeScale(Dec);

impl FeeScale {
    pub const ZERO: FeeScale = FeeScale(Dec::ZERO);
    pub const ONE: FeeScale = FeeScale(Dec::ONE);

    #[must_use]
    pub const fn value(self) -> Dec {
        self.0
    }

    /// The string hashed: no trailing zeros.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_wire()
    }
}

impl FromStr for FeeScale {
    type Err = DeployError;

    /// Takes `1.0`, `01` or `.50`, and keeps what the venue hashes: `1`, `1`,
    /// `0.5`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        bounded(text, Dec::from_int(10)).map(FeeScale).ok_or_else(|| DeployError::FeeScale(text.trim().to_owned()))
    }
}

impl fmt::Display for FeeScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The share of an outcome's pool its first side is paid when it settles,
/// from 0 to 1: `1` pays the first side in full and `0` the second. A
/// standalone outcome settles to any fraction; a question's only to 0 or 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction(Dec);

impl Fraction {
    pub const ZERO: Fraction = Fraction(Dec::ZERO);
    pub const ONE: Fraction = Fraction(Dec::ONE);

    #[must_use]
    pub const fn value(self) -> Dec {
        self.0
    }

    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_wire()
    }

    /// Exactly 0 or exactly 1, the only fractions a question's outcome takes.
    #[must_use]
    pub fn is_whole(self) -> bool {
        self == Fraction::ZERO || self == Fraction::ONE
    }
}

impl FromStr for Fraction {
    type Err = DeployError;

    /// As [`FeeScale`]'s: `1.0` is kept as `1` and `.50` as `0.5`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        bounded(text, Dec::ONE).map(Fraction).ok_or_else(|| DeployError::Fraction(text.trim().to_owned()))
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
#[path = "tests/bounded.rs"]
mod tests;
