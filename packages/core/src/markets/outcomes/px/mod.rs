//! Outcome prices and sizes as whole numbers. A [`Px`] is a probability in ticks of 0.00001, on the
//! grid or refused, never rounded unasked; a [`Qty`] is whole contracts (`szDecimals = 0`) and a
//! [`SignedQty`] a holding that may be short; a [`Mid`] is a reported price at eight places, which
//! can sit on a half tick.

pub mod mid;
pub mod qty;
pub mod signed_qty;

pub use mid::{Mid, OBSERVED_PER_UNIT, OBSERVED_PLACES};
pub use qty::{BadQty, Qty};
pub use signed_qty::{BadSignedQty, SignedQty};

use std::{fmt, str::FromStr};

use crate::markets::outcomes::coin::PRICE_PLACES;
use crate::utils::{Dec, Round};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Ticks in one unit of the quote token: a tick is 0.00001.
pub const TICKS_PER_UNIT: u32 = 100_000;

/// Why a figure is not a price.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadPx {
    /// Not a decimal, or a negative one; holds the text.
    #[error("`{0}` is not a decimal number")]
    NotANumber(String),
    /// Above 1; holds the text.
    #[error("price `{0}` is outside 0 to 1")]
    OutOfRange(String),
    /// More than five decimal places; holds the text.
    #[error("price `{0}` is finer than the 0.00001 tick")]
    TooFine(String),
}

/// A probability in `[0, 1]`, counted in ticks of 0.00001. Only [`Px::MIN_QUOTE`] to
/// [`Px::MAX_QUOTE`] may be ordered at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Px(u32);

impl Px {
    /// A price of 0: the side pays nothing.
    pub const ZERO: Px = Px(0);
    /// A price of 1: the side pays in full.
    pub const ONE: Px = Px(TICKS_PER_UNIT);
    /// The lowest price an order may name: a market sell goes at it.
    pub const MIN_QUOTE: Px = Px(1);
    /// The highest price an order may name: a market buy goes at it.
    pub const MAX_QUOTE: Px = Px(TICKS_PER_UNIT - 1);

    /// `None` past one.
    #[must_use]
    pub const fn from_ticks(ticks: u32) -> Option<Px> {
        match ticks <= TICKS_PER_UNIT {
            true => Some(Px(ticks)),
            false => None,
        }
    }

    /// One past one, clamped to one.
    #[must_use]
    pub const fn saturating_from_ticks(ticks: u32) -> Px {
        match ticks <= TICKS_PER_UNIT {
            true => Px(ticks),
            false => Px::ONE,
        }
    }

    /// The price in ticks of 0.00001.
    #[must_use]
    pub const fn ticks(self) -> u32 {
        self.0
    }

    /// The other side's price: `1 - p`.
    #[must_use]
    pub const fn complement(self) -> Px {
        Px(TICKS_PER_UNIT - self.0)
    }

    /// Whether the price is 0.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Whether an order may name it: 0.00001 to 0.99999.
    #[must_use]
    pub const fn is_quotable(self) -> bool {
        self.0 >= Px::MIN_QUOTE.0 && self.0 <= Px::MAX_QUOTE.0
    }

    /// The nearest price an order may name.
    #[must_use]
    pub const fn clamp_to_quotable(self) -> Px {
        if self.0 < Px::MIN_QUOTE.0 {
            Px::MIN_QUOTE
        } else if self.0 > Px::MAX_QUOTE.0 {
            Px::MAX_QUOTE
        } else {
            self
        }
    }

    /// Half way between, rounded down to a tick.
    #[must_use]
    pub const fn midpoint(self, other: Px) -> Px {
        Px((self.0 + other.0) / 2)
    }

    /// `None` past one.
    #[must_use]
    pub const fn checked_add(self, other: Px) -> Option<Px> {
        Px::from_ticks(self.0 + other.0)
    }

    /// How many ticks apart the two prices are.
    #[must_use]
    pub const fn abs_diff(self, other: Px) -> u32 {
        self.0.abs_diff(other.0)
    }

    /// `ticks` along the grid, stopping at 0 and 1.
    #[must_use]
    pub const fn step(self, ticks: i32) -> Px {
        let moved = self.0 as i64 + ticks as i64;
        if moved < 0 {
            Px::ZERO
        } else if moved > TICKS_PER_UNIT as i64 {
            Px::ONE
        } else {
            Px(moved as u32)
        }
    }

    /// Reads a price exactly: `0.27`, `0.2700000`. A figure past five places is refused, not
    /// rounded.
    pub fn parse(text: &str) -> Result<Px, BadPx> {
        let value = Dec::parse(text).map_err(|_| BadPx::NotANumber(text.to_owned()))?;
        if value.is_negative() {
            return Err(BadPx::NotANumber(text.to_owned()));
        }
        Px::exact(value).map_err(|why| why.naming(text))
    }

    /// A decimal exactly on the grid.
    pub fn from_dec(value: Dec) -> Result<Px, BadPx> {
        Px::exact(value).map_err(|why| why.naming(&value.to_string()))
    }

    fn exact(value: Dec) -> Result<Px, BadPx> {
        if value.places() > PRICE_PLACES {
            return Err(BadPx::TooFine(String::new()));
        }
        Px::on_grid(value, Round::Down).ok_or(BadPx::OutOfRange(String::new()))
    }

    /// `value` put on the grid the way asked, or `None` when that lands outside 0 to 1.
    #[must_use]
    pub fn on_grid(value: Dec, round: Round) -> Option<Px> {
        let ticks = value.units_at(PRICE_PLACES, round)?;
        u32::try_from(ticks).ok().and_then(Px::from_ticks)
    }

    /// The nearest grid price, a half tick rounding up.
    #[must_use]
    pub fn nearest(value: Dec) -> Option<Px> {
        Px::on_grid(value, Round::Nearest)
    }

    /// The price as a decimal with five places.
    #[must_use]
    pub const fn to_dec(self) -> Dec {
        Dec::lit::<PRICE_PLACES>(self.0 as i128)
    }

    /// No trailing zeros: `0.27`, `1`.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.to_dec().to_wire()
    }
}

impl BadPx {
    fn naming(self, text: &str) -> BadPx {
        let text = text.to_owned();
        match self {
            BadPx::NotANumber(_) => BadPx::NotANumber(text),
            BadPx::OutOfRange(_) => BadPx::OutOfRange(text),
            BadPx::TooFine(_) => BadPx::TooFine(text),
        }
    }
}

impl fmt::Display for Px {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dec())
    }
}

impl FromStr for Px {
    type Err = BadPx;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Px::parse(text)
    }
}

/// From the venue's price strings.
impl<'de> Deserialize<'de> for Px {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Px::parse(&text).map_err(de::Error::custom)
    }
}

/// As the wire writes it: a string, the way [`Px::parse`] reads it.
impl Serialize for Px {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.to_wire())
    }
}

#[cfg(test)]
#[path = "tests/px.rs"]
mod tests;
