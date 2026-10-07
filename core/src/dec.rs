use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// The finest figure this type keeps. The venue's own ceiling is eight
/// places for a spot amount; marks, funding and ETH in wei run deeper.
pub const MAX_SCALE: u32 = 18;

/// An exact decimal, `units / 10^scale`, for every amount the venue prices
/// in. The venue sends and takes these as strings, and a float between the
/// two would change the figure that gets signed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dec {
    units: i128,
    scale: u32,
}

/// Which way a figure moves when it has more digits than it may keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    /// Toward zero, which is what the venue's own clients do to an amount.
    Down,
    /// Away from zero.
    Up,
    /// To the nearer, and away from zero on a tie.
    Nearest,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadDec {
    #[error("`{0}` is not a decimal number")]
    NotANumber(String),
    #[error("`{0}` has more than eighteen decimal places")]
    TooFine(String),
    #[error("`{0}` is larger than this build can count")]
    OutOfRange(String),
}

impl Dec {
    pub const ZERO: Dec = Dec { units: 0, scale: 0 };
    pub const ONE: Dec = Dec { units: 1, scale: 0 };

    /// `units / 10^scale`. A scale past [`MAX_SCALE`] is refused.
    #[must_use]
    pub const fn new(units: i128, scale: u32) -> Option<Dec> {
        if scale > MAX_SCALE {
            return None;
        }
        Some(Dec { units, scale })
    }

    #[must_use]
    pub const fn from_int(whole: i64) -> Dec {
        Dec { units: whole as i128, scale: 0 }
    }

    /// Reads what the venue writes: an optional sign, digits, and at most one
    /// point. No exponent, since the venue never sends one.
    pub fn parse(text: &str) -> Result<Dec, BadDec> {
        let bad = || BadDec::NotANumber(text.to_owned());
        let body = text.trim();
        let (negative, body) = match body.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, body.strip_prefix('+').unwrap_or(body)),
        };
        let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
        if whole.is_empty() && fraction.is_empty() {
            return Err(bad());
        }
        if !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let fraction = fraction.trim_end_matches('0');
        let scale = u32::try_from(fraction.len()).map_err(|_| BadDec::TooFine(text.to_owned()))?;
        if scale > MAX_SCALE {
            return Err(BadDec::TooFine(text.to_owned()));
        }
        let mut units: i128 = 0;
        for digit in whole.bytes().chain(fraction.bytes()) {
            units = units
                .checked_mul(10)
                .and_then(|u| u.checked_add(i128::from(digit - b'0')))
                .ok_or_else(|| BadDec::OutOfRange(text.to_owned()))?;
        }
        Ok(Dec { units: if negative { -units } else { units }, scale }.normal())
    }

    /// The same value with no trailing zeros behind the point.
    #[must_use]
    fn normal(mut self) -> Dec {
        while self.scale > 0 && self.units % 10 == 0 {
            self.units /= 10;
            self.scale -= 1;
        }
        if self.units == 0 {
            self.scale = 0;
        }
        self
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.units == 0
    }

    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.units < 0
    }

    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.units > 0
    }

    #[must_use]
    pub fn is_integer(self) -> bool {
        self.normal().scale == 0
    }

    /// How many places behind the point the value needs.
    #[must_use]
    pub fn places(self) -> u32 {
        self.normal().scale
    }

    #[must_use]
    pub const fn abs(self) -> Dec {
        Dec { units: self.units.abs(), scale: self.scale }
    }

    #[must_use]
    pub const fn negate(self) -> Dec {
        Dec { units: -self.units, scale: self.scale }
    }

    /// The value as a count of `10^-places`, rounded as asked. An integer
    /// amount on a chain (USDC in millionths, say) comes out of here.
    #[must_use]
    pub fn units_at(self, places: u32, round: Round) -> Option<i128> {
        match places.cmp(&self.scale) {
            Ordering::Equal => Some(self.units),
            Ordering::Greater => self.units.checked_mul(ten(places - self.scale)?),
            Ordering::Less => Some(divide(self.units, ten(self.scale - places)?, round)),
        }
    }

    /// Kept to at most `places` behind the point.
    #[must_use]
    pub fn round(self, places: u32, round: Round) -> Dec {
        if places >= self.scale {
            return self.normal();
        }
        self.units_at(places, round).map_or(self, |units| Dec { units, scale: places }.normal())
    }

    /// Kept to at most `figures` significant digits. Digits dropped before the
    /// point become zeros, so `123456.7` to five figures is `123450`.
    #[must_use]
    pub fn round_sig(self, figures: u32, round: Round) -> Dec {
        let digits = digits(self.units);
        if figures == 0 || digits <= figures {
            return self.normal();
        }
        let dropped = digits - figures;
        let Some(step) = ten(dropped) else {
            return self;
        };
        let kept = divide(self.units, step, round);
        match kept.checked_mul(step) {
            Some(units) => Dec { units, scale: self.scale }.normal(),
            None => self,
        }
    }

    fn aligned(self, other: Dec) -> Option<(i128, i128, u32)> {
        let scale = self.scale.max(other.scale);
        Some((
            self.units.checked_mul(ten(scale - self.scale)?)?,
            other.units.checked_mul(ten(scale - other.scale)?)?,
            scale,
        ))
    }

    #[must_use]
    pub fn checked_add(self, other: Dec) -> Option<Dec> {
        let (a, b, scale) = self.aligned(other)?;
        Some(Dec { units: a.checked_add(b)?, scale }.normal())
    }

    #[must_use]
    pub fn checked_sub(self, other: Dec) -> Option<Dec> {
        self.checked_add(other.negate())
    }

    /// Exact while the product fits eighteen places; past that, the nearest.
    #[must_use]
    pub fn checked_mul(self, other: Dec) -> Option<Dec> {
        let (a, b) = (self.normal(), other.normal());
        let units = a.units.checked_mul(b.units)?;
        let scale = a.scale + b.scale;
        if scale <= MAX_SCALE {
            return Some(Dec { units, scale }.normal());
        }
        let units = divide(units, ten(scale - MAX_SCALE)?, Round::Nearest);
        Some(Dec { units, scale: MAX_SCALE }.normal())
    }

    /// `self / other` kept to `places` behind the point.
    #[must_use]
    pub fn checked_div(self, other: Dec, places: u32, round: Round) -> Option<Dec> {
        if other.units == 0 || places > MAX_SCALE {
            return None;
        }
        let (a, b) = (self.normal(), other.normal());
        // a/b = (ua * 10^(places + sb)) / (ub * 10^sa), in units of 10^-places.
        let numerator = a.units.checked_mul(ten(places + b.scale)?)?;
        let denominator = b.units.checked_mul(ten(a.scale)?)?;
        let (numerator, denominator) = match denominator < 0 {
            true => (numerator.checked_neg()?, denominator.checked_neg()?),
            false => (numerator, denominator),
        };
        Some(Dec { units: divide(numerator, denominator, round), scale: places }.normal())
    }

    /// The canonical spelling: no trailing zeros, no exponent, `0` for zero.
    /// This is the string that is hashed, so it must match the venue's clients.
    #[must_use]
    pub fn to_wire(self) -> String {
        self.to_string()
    }
}

/// `10^power`, or `None` past what an `i128` holds.
fn ten(power: u32) -> Option<i128> {
    10i128.checked_pow(power)
}

fn digits(units: i128) -> u32 {
    let mut left = units.unsigned_abs();
    let mut count = 0;
    while left > 0 {
        left /= 10;
        count += 1;
    }
    count
}

/// Integer division with the rounding asked for; `by` is positive.
fn divide(value: i128, by: i128, round: Round) -> i128 {
    let quotient = value / by;
    let rest = value % by;
    if rest == 0 {
        return quotient;
    }
    let away = if value < 0 { -1 } else { 1 };
    match round {
        Round::Down => quotient,
        Round::Up => quotient + away,
        Round::Nearest => match (rest.unsigned_abs() * 2).cmp(&by.unsigned_abs()) {
            Ordering::Less => quotient,
            _ => quotient + away,
        },
    }
}

impl PartialEq for Dec {
    fn eq(&self, other: &Dec) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dec {}

impl PartialOrd for Dec {
    fn partial_cmp(&self, other: &Dec) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dec {
    fn cmp(&self, other: &Dec) -> Ordering {
        match self.aligned(*other) {
            Some((a, b, _)) => a.cmp(&b),
            // Only a figure near the i128 ceiling lands here; its sign and
            // integer part decide it.
            None => (self.units / ten(self.scale).unwrap_or(1)).cmp(&(other.units / ten(other.scale).unwrap_or(1))),
        }
    }
}

impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let normal = self.normal();
        let sign = if normal.units < 0 { "-" } else { "" };
        let magnitude = normal.units.unsigned_abs();
        if normal.scale == 0 {
            return write!(f, "{sign}{magnitude}");
        }
        let step = 10u128.pow(normal.scale);
        let places = normal.scale as usize;
        write!(f, "{sign}{}.{:0places$}", magnitude / step, magnitude % step)
    }
}

impl FromStr for Dec {
    type Err = BadDec;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Dec::parse(text)
    }
}

impl Serialize for Dec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// From the venue's decimal strings, or a JSON integer. A JSON fraction is
/// refused: it has already been through a float.
impl<'de> Deserialize<'de> for Dec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = Dec;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a decimal string or an integer")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Dec, E> {
                Dec::parse(text).map_err(E::custom)
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Dec, E> {
                Ok(Dec { units: i128::from(value), scale: 0 })
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Dec, E> {
                Ok(Dec { units: i128::from(value), scale: 0 })
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[cfg(test)]
#[path = "tests/dec.rs"]
mod tests;
