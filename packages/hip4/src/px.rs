//! Outcome prices and sizes as whole numbers (D1, D2).
//!
//! A price is a probability counted in ticks of 0.00001: the venue takes at
//! most five places, every level on an outcome book sits on that grid, and a
//! side's price `p` and the other side's `1 - p` share it. A value is on the
//! grid or it is not a price; nothing is rounded without being asked. A size
//! is whole contracts: outcome books quote `szDecimals = 0`.
//!
//! What the venue reports is finer than what can be traded: a mid is the
//! midpoint of two grid prices and lands on a half tick about a third of the
//! time, so it is a [`Mid`], at eight places, and says which way it goes when
//! it is brought onto the grid.

use std::{fmt, str::FromStr};

use hl_core::{Dec, Round};
use hl_trade::MINIMUM_ORDER_VALUE;
use serde::{Deserialize, Deserializer, de};

use crate::coin::PRICE_PLACES;

/// Ticks in one unit of the quote token: a tick is 0.00001.
pub const TICKS_PER_UNIT: u32 = 100_000;

/// Places a reported mid keeps.
pub const OBSERVED_PLACES: u32 = 8;

/// Units of a reported mid in one unit of the quote token.
pub const OBSERVED_PER_UNIT: u32 = 100_000_000;

/// hip4-sdk's minimum order (`MIN_NOTIONAL` in `pricing.ts`): 1.5 of the
/// quote token, each share valued at its short side ([`min_shares`]). The
/// venue publishes no figure; hip4-sdk says its floor is 1 and keeps half
/// of one as headroom.
pub const MIN_NOTIONAL: Dec = match Dec::new(15, 1) {
    Some(least) => least,
    None => Dec::ZERO,
};

/// A share is never valued under a cent for [`min_shares`].
const SHORT_SIDE_FLOOR: Dec = match Dec::new(1, 2) {
    Some(cent) => cent,
    None => Dec::ZERO,
};

/// Why a figure is not a price.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadPx {
    #[error("`{0}` is not a decimal number")]
    NotANumber(String),
    #[error("price `{0}` is outside 0 to 1")]
    OutOfRange(String),
    #[error("price `{0}` is finer than the 0.00001 tick")]
    TooFine(String),
}

/// A probability in `[0, 1]`, counted in ticks of 0.00001. Only
/// [`Px::MIN_QUOTE`] to [`Px::MAX_QUOTE`] may be ordered at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Px(u32);

impl Px {
    pub const ZERO: Px = Px(0);
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

    #[must_use]
    pub const fn ticks(self) -> u32 {
        self.0
    }

    /// The other side's price: `1 - p`.
    #[must_use]
    pub const fn complement(self) -> Px {
        Px(TICKS_PER_UNIT - self.0)
    }

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

    /// Reads a price exactly: `0.27`, `0.2700000`. A figure past five places
    /// is refused, not rounded.
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

    /// `value` put on the grid the way asked, or `None` when that lands
    /// outside 0 to 1.
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

    #[must_use]
    pub const fn to_dec(self) -> Dec {
        match Dec::new(self.0 as i128, PRICE_PLACES) {
            Some(value) => value,
            None => Dec::ZERO,
        }
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

/// A price the venue reports, at eight places, that may not be tradable: a
/// mid, a mark.
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
        match Dec::new(self.0 as i128, OBSERVED_PLACES) {
            Some(value) => value,
            None => Dec::ZERO,
        }
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

/// Why a figure is not a size.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadQty {
    #[error("`{0}` is not a whole number of contracts")]
    NotAnInteger(String),
    #[error("size `{0}` is fractional; outcome books trade whole contracts")]
    Fractional(String),
}

/// Whole contracts. A fractional size is a caller's error, said as one, not
/// rounded into an order the venue refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Qty(u64);

impl Qty {
    pub const ZERO: Qty = Qty(0);

    #[must_use]
    pub const fn new(contracts: u64) -> Qty {
        Qty(contracts)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn min(self, other: Qty) -> Qty {
        if self.0 < other.0 { self } else { other }
    }

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

    #[must_use]
    pub const fn to_dec(self) -> Dec {
        match Dec::new(self.0 as i128, 0) {
            Some(value) => value,
            None => Dec::ZERO,
        }
    }

    #[must_use]
    pub fn to_wire(self) -> String {
        self.0.to_string()
    }

    /// What these contracts cost at `price`, in the quote token, exactly.
    #[must_use]
    pub const fn notional(self, price: Px) -> Dec {
        match Dec::new(self.0 as i128 * price.0 as i128, PRICE_PLACES) {
            Some(value) => value,
            None => Dec::ZERO,
        }
    }

    /// hip4-tui's minimum: whether these are worth the spot minimum of 10 of
    /// the quote token at `price` (a market order valued at the edge it goes
    /// at).
    #[must_use]
    pub fn worth_minimum(self, price: Px) -> bool {
        self.notional(price) >= MINIMUM_ORDER_VALUE
    }

    /// The fewest contracts [`Qty::worth_minimum`] at `price`; `None` at
    /// zero, where nothing is worth anything.
    #[must_use]
    pub fn least_at(price: Px) -> Option<Qty> {
        if price.is_zero() {
            return None;
        }
        whole(MINIMUM_ORDER_VALUE.checked_div(price.to_dec(), 0, Round::Up)?)
    }

    /// The whole contracts `amount` of the quote token buys at `price`,
    /// toward zero. `None` at a zero price or for a negative amount.
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

/// hip4-sdk's `getMinShares`: the fewest shares an order at `mark` may have,
/// `ceil(1.5 / clamp(min(p, 1 - p), 0.01, 1))`, each share valued at its
/// short side and never under a cent. Exact, where the TypeScript's float
/// `1 - 0.9` asks 16 at 0.9 instead of 15.
#[must_use]
pub fn min_shares(mark: Dec) -> Qty {
    let most = Qty(150);
    let Some(other) = Dec::ONE.checked_sub(mark) else {
        return most;
    };
    let short = mark.min(other).clamp(SHORT_SIDE_FLOOR, Dec::ONE);
    MIN_NOTIONAL.checked_div(short, 0, Round::Up).and_then(whole).unwrap_or(most)
}

/// Why a figure is not a holding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadSignedQty {
    #[error(transparent)]
    Size(#[from] BadQty),
    #[error("holding `{0}` is more contracts than this build can count")]
    TooLarge(String),
}

/// Whole contracts that may be short: what fills say is held in a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SignedQty(i64);

impl SignedQty {
    pub const ZERO: SignedQty = SignedQty(0);

    #[must_use]
    pub const fn new(contracts: i64) -> SignedQty {
        SignedQty(contracts)
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn is_short(self) -> bool {
        self.0 < 0
    }

    #[must_use]
    pub const fn abs(self) -> Qty {
        Qty(self.0.unsigned_abs())
    }

    /// Saturates at what an `i64` counts.
    #[must_use]
    pub const fn long(size: Qty) -> SignedQty {
        match size.0 > i64::MAX as u64 {
            true => SignedQty(i64::MAX),
            false => SignedQty(size.0 as i64),
        }
    }

    #[must_use]
    pub const fn short(size: Qty) -> SignedQty {
        match size.0 > i64::MAX as u64 {
            true => SignedQty(i64::MIN),
            false => SignedQty(-(size.0 as i64)),
        }
    }

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
        let contracts = i64::try_from(size.0).map_err(|_| BadSignedQty::TooLarge(raw.to_owned()))?;
        Ok(SignedQty(if negative { -contracts } else { contracts }))
    }

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

#[cfg(test)]
#[path = "tests/px.rs"]
mod tests;
