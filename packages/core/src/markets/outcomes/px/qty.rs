//! A size on an outcome book: whole contracts, what they cost at a price, and the fewest an order
//! must carry to be worth the spot minimum.

use std::{fmt, str::FromStr};

use crate::markets::outcomes::coin::PRICE_PLACES;
use crate::markets::outcomes::px::Px;
use crate::utils::MINIMUM_ORDER_VALUE;
use crate::utils::{Dec, Round};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Why a figure is not a size.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BadQty {
    /// Not a number of contracts: a sign other than `+`, a character that is not a digit, or too many
    /// contracts to count; holds the text.
    #[error("`{0}` is not a whole number of contracts")]
    NotAnInteger(String),
    /// A size with a fractional part; holds the text.
    #[error("size `{0}` is fractional; outcome books trade whole contracts")]
    Fractional(String),
}

/// Whole contracts. A fractional size is refused as an error, not rounded into an order the venue
/// refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Qty(u64);

impl Qty {
    /// No contracts.
    pub const ZERO: Qty = Qty(0);

    /// `contracts` whole contracts.
    #[must_use]
    pub const fn new(contracts: u64) -> Qty {
        Qty(contracts)
    }

    /// The number of contracts.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Whether there are no contracts.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// The smaller of the two sizes.
    #[must_use]
    pub const fn min(self, other: Qty) -> Qty {
        if self.0 < other.0 { self } else { other }
    }

    /// The difference, stopping at zero.
    #[must_use]
    pub const fn saturating_sub(self, other: Qty) -> Qty {
        Qty(self.0.saturating_sub(other.0))
    }

    /// Reads a size, taking the `544.0` a book prints.
    pub fn parse(text: &str) -> Result<Qty, BadQty> {
        let raw = text.trim();
        let body = raw.strip_prefix('+').unwrap_or(raw);
        let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty() || !digits(whole) || !digits(fraction) {
            return Err(BadQty::NotAnInteger(raw.to_owned()));
        }
        if fraction.bytes().any(|byte| byte != b'0') {
            return Err(BadQty::Fractional(raw.to_owned()));
        }
        whole.parse().map(Qty).map_err(|_| BadQty::NotAnInteger(raw.to_owned()))
    }

    /// A decimal that is a whole number of contracts.
    pub fn from_dec(value: Dec) -> Result<Qty, BadQty> {
        if !value.is_integer() {
            return Err(BadQty::Fractional(value.to_string()));
        }
        value
            .units_at(0, Round::Down)
            .and_then(|whole| u64::try_from(whole).ok())
            .map(Qty)
            .ok_or_else(|| BadQty::NotAnInteger(value.to_string()))
    }

    /// The size as a whole decimal.
    #[must_use]
    pub const fn to_dec(self) -> Dec {
        Dec::lit::<0>(self.0 as i128)
    }

    /// The size as the wire writes it: digits, no point.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_string()
    }

    /// What these contracts cost at `price`, in the quote token, exactly.
    #[must_use]
    pub const fn notional(self, price: Px) -> Dec {
        Dec::lit::<PRICE_PLACES>(self.0 as i128 * price.0 as i128)
    }

    /// Whether these are worth the spot minimum of 10 of the quote token at `price` (a market order
    /// is valued at its bound).
    #[must_use]
    pub fn worth_minimum(self, price: Px) -> bool {
        self.notional(price) >= MINIMUM_ORDER_VALUE
    }

    /// The fewest contracts [`Qty::worth_minimum`] at `price`; `None` at zero.
    #[must_use]
    pub fn least_at(price: Px) -> Option<Qty> {
        if price.is_zero() {
            return None;
        }
        whole(MINIMUM_ORDER_VALUE.checked_div(price.to_dec(), 0, Round::Up)?)
    }

    /// The whole contracts `amount` of the quote token buys at `price`, toward zero. `None` at a
    /// zero price or for a negative amount.
    #[must_use]
    pub fn spend(amount: Dec, price: Px) -> Option<Qty> {
        if price.is_zero() || amount.is_negative() {
            return None;
        }
        whole(amount.checked_div(price.to_dec(), 0, Round::Down)?)
    }
}

fn whole(value: Dec) -> Option<Qty> {
    value.units_at(0, Round::Down).and_then(|whole| u64::try_from(whole).ok()).map(Qty)
}

impl fmt::Display for Qty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Qty {
    type Err = BadQty;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Qty::parse(text)
    }
}

impl<'de> Deserialize<'de> for Qty {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Qty::parse(&text).map_err(de::Error::custom)
    }
}

/// As the wire writes it: a string, the way [`Qty::parse`] reads it.
impl Serialize for Qty {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.to_wire())
    }
}

#[cfg(test)]
#[path = "tests/qty.rs"]
mod tests;
