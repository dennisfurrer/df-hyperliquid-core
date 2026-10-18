//! A price the venue reports at eight places, a mid or a mark, which may sit between two ticks and
//! so may not be one an order can name.

use std::{fmt, str::FromStr};

use crate::markets::outcomes::px::{BadPx, Px, TICKS_PER_UNIT};
use crate::utils::{Dec, Round};
use serde::{Deserialize, Deserializer, de};

/// Places a reported mid keeps.
pub const OBSERVED_PLACES: u32 = 8;

/// Units of a reported mid in one unit of the quote token.
pub const OBSERVED_PER_UNIT: u32 = 100_000_000;

/// A price the venue reports at eight places, a mid or a mark, that may not be tradable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Mid(u32);

impl Mid {
    pub const ZERO: Mid = Mid(0);
    pub const ONE: Mid = Mid(OBSERVED_PER_UNIT);

    /// Units of 0.00000001 between 0 and one.
    #[must_use]
    pub const fn units(self) -> u32 {
        self.0
    }

    /// `None` past one.
    #[must_use]
    pub const fn from_units(units: u32) -> Option<Mid> {
        match units <= OBSERVED_PER_UNIT {
            true => Some(Mid(units)),
            false => None,
        }
    }

    #[must_use]
    pub const fn complement(self) -> Mid {
        Mid(OBSERVED_PER_UNIT - self.0)
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Every price is a mid.
    #[must_use]
    pub const fn of(price: Px) -> Mid {
        Mid(price.0 * (OBSERVED_PER_UNIT / TICKS_PER_UNIT))
    }

    /// Whether it sits on the grid.
    #[must_use]
    pub const fn is_orderable(self) -> bool {
        self.0.is_multiple_of(OBSERVED_PER_UNIT / TICKS_PER_UNIT)
    }

    /// The nearest grid price; a half tick rounds up.
    #[must_use]
    pub const fn to_px_nearest(self) -> Px {
        let per = OBSERVED_PER_UNIT / TICKS_PER_UNIT;
        Px((self.0 + per / 2) / per)
    }

    #[must_use]
    pub const fn to_px_floor(self) -> Px {
        Px(self.0 / (OBSERVED_PER_UNIT / TICKS_PER_UNIT))
    }

    #[must_use]
    pub const fn to_px_ceil(self) -> Px {
        Px(self.0.div_ceil(OBSERVED_PER_UNIT / TICKS_PER_UNIT))
    }

    /// Reads a reported price exactly, to eight places.
    pub fn parse(text: &str) -> Result<Mid, BadPx> {
        let value = Dec::parse(text).map_err(|_| BadPx::NotANumber(text.to_owned()))?;
        if value.is_negative() {
            return Err(BadPx::NotANumber(text.to_owned()));
        }
        if value.places() > OBSERVED_PLACES {
            return Err(BadPx::TooFine(text.to_owned()));
        }
        value
            .units_at(OBSERVED_PLACES, Round::Down)
            .and_then(|units| u32::try_from(units).ok())
            .filter(|units| *units <= OBSERVED_PER_UNIT)
            .map(Mid)
            .ok_or_else(|| BadPx::OutOfRange(text.to_owned()))
    }

    #[must_use]
    pub const fn to_dec(self) -> Dec {
        Dec::lit::<OBSERVED_PLACES>(self.0 as i128)
    }

    #[must_use]
    pub fn to_wire(self) -> String {
        self.to_dec().to_wire()
    }
}

impl fmt::Display for Mid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dec())
    }
}

impl FromStr for Mid {
    type Err = BadPx;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Mid::parse(text)
    }
}

impl<'de> Deserialize<'de> for Mid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Mid::parse(&text).map_err(de::Error::custom)
    }
}

#[cfg(test)]
#[path = "tests/mid.rs"]
mod tests;
