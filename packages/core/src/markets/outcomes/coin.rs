//! An outcome side (`#<encoding>`), its asset id, and the orders on it: an [`Order`] built from a
//! grid price ([`Px`]) and whole contracts ([`Qty`]).

use std::{fmt, str::FromStr};

use crate::markets::outcomes::error::OutcomeError;
use crate::markets::outcomes::px::{Px, Qty};
use crate::trade::{Order, Side, Tif};
use crate::utils::MAX_SLIPPAGE;
use crate::utils::{Asset, Dec, OutcomeSide};

/// Places an outcome price keeps: a tick is 0.00001.
pub const PRICE_PLACES: u32 = 5;

/// One side of an outcome: `10 * outcome + side`, spelled `#<encoding>` in info and on the
/// websocket and `+<encoding>` in spot balances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Coin {
    outcome: u32,
    side: OutcomeSide,
}

impl Coin {
    #[must_use]
    pub const fn new(outcome: u32, side: OutcomeSide) -> Coin {
        Coin { outcome, side }
    }

    #[must_use]
    pub const fn outcome(self) -> u32 {
        self.outcome
    }

    #[must_use]
    pub const fn side(self) -> OutcomeSide {
        self.side
    }

    /// From `10 * outcome + side`; `None` when its last digit is neither side or the outcome is
    /// too large.
    #[must_use]
    pub const fn from_encoding(encoding: u64) -> Option<Coin> {
        let side = match encoding % 10 {
            0 => OutcomeSide::Yes,
            1 => OutcomeSide::No,
            _ => return None,
        };
        let outcome = encoding / 10;
        match outcome <= u32::MAX as u64 {
            true => Some(Coin { outcome: outcome as u32, side }),
            false => None,
        }
    }

    /// `10 * outcome + side`.
    #[must_use]
    pub const fn encoding(self) -> u64 {
        self.outcome as u64 * 10 + self.side.digit() as u64
    }

    #[must_use]
    pub const fn opposite(self) -> Coin {
        Coin { outcome: self.outcome, side: self.side.other() }
    }

    /// Asset id `100000000 + encoding`; `None` only for an outcome too large to encode.
    #[must_use]
    pub const fn asset(self) -> Option<Asset> {
        Asset::outcome(self.outcome, self.side)
    }

    #[must_use]
    pub const fn from_asset(asset: Asset) -> Option<Coin> {
        match asset.outcome_side() {
            Some((outcome, side)) => Some(Coin { outcome, side }),
            None => None,
        }
    }

    /// `+12100`, as a spot balance names it.
    #[must_use]
    pub fn balance_name(self) -> String {
        format!("+{}", self.encoding())
    }

    /// Reads `#12100` or `+12100`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Coin> {
        let text = text.trim();
        let digits = text.strip_prefix('#').or_else(|| text.strip_prefix('+'))?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        Coin::from_encoding(digits.parse().ok()?)
    }

    /// A market order: `FrontendMarket` at the edge of the range, 0.99999 to buy and 0.00001 to
    /// sell, so it crosses whatever the book holds and never rests. Never `Gtc`. Zero contracts is
    /// refused.
    pub fn market(self, side: Side, size: Qty) -> Result<Order, OutcomeError> {
        let asset = self.asset().ok_or(OutcomeError::NoAsset(self))?;
        if size.is_zero() {
            return Err(OutcomeError::Size(Dec::ZERO));
        }
        let bound = match side {
            Side::Buy => Px::MAX_QUOTE,
            Side::Sell => Px::MIN_QUOTE,
        };
        Ok(Order::market(asset, side, bound.to_dec(), size.to_dec()))
    }

    /// A limit order at a price already on the grid. A price of 0 or 1, or zero contracts, is
    /// refused.
    pub fn order(self, side: Side, price: Px, size: Qty, tif: Tif) -> Result<Order, OutcomeError> {
        let asset = self.asset().ok_or(OutcomeError::NoAsset(self))?;
        if !price.is_quotable() {
            return Err(OutcomeError::Price(price.to_dec()));
        }
        if size.is_zero() {
            return Err(OutcomeError::Size(Dec::ZERO));
        }
        Ok(Order::limit(asset, side, price.to_dec(), size.to_dec(), tif))
    }

    /// A market order bounded `slippage` (a fraction, at most 0.5) past `reference`. The bound is
    /// rounded onto the grid toward the reference, so never looser than asked, and stops at the
    /// edge of the range. `FrontendMarket`, never `Gtc`.
    pub fn market_within(self, side: Side, reference: Px, slippage: Dec, size: Qty) -> Result<Order, OutcomeError> {
        let asset = self.asset().ok_or(OutcomeError::NoAsset(self))?;
        if slippage.is_negative() || slippage > MAX_SLIPPAGE {
            return Err(OutcomeError::Slippage(slippage));
        }
        if size.is_zero() {
            return Err(OutcomeError::Size(Dec::ZERO));
        }
        let (factor, past) = match side {
            Side::Buy => (Dec::ONE.checked_add(slippage), Px::ONE),
            Side::Sell => (Dec::ONE.checked_sub(slippage), Px::ZERO),
        };
        let moved = factor
            .and_then(|factor| reference.to_dec().checked_mul(factor))
            .ok_or(OutcomeError::Price(reference.to_dec()))?;
        let bound = Px::on_grid(moved, side.price_rounding()).unwrap_or(past).clamp_to_quotable();
        Ok(Order::market(asset, side, bound.to_dec(), size.to_dec()))
    }
}

/// `#12100`, as info and the websocket name it.
impl fmt::Display for Coin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.encoding())
    }
}

/// As [`fmt::Display`] writes it, `#12100`; either spelling reads back.
impl serde::Serialize for Coin {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Coin {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl FromStr for Coin {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Coin::parse(text).ok_or_else(|| format!("`{text}` is not an outcome coin; expected `#12100`"))
    }
}

#[cfg(test)]
#[path = "tests/coin.rs"]
mod tests;
