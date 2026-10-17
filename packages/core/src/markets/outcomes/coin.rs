//! An outcome side, and the grid its orders sit on: prices in ticks of 0.00001 from 0.00001 to
//! 0.99999, sizes in whole contracts.

use std::{fmt, str::FromStr};

use crate::markets::outcomes::error::OutcomeError;
use crate::markets::outcomes::px::{Px, Qty};
use crate::trade::{Order, Side, Tif};
use crate::utils::MAX_SLIPPAGE;
use crate::utils::{Asset, Dec, OutcomeSide, Round};

/// Places an outcome price keeps: a tick is 0.00001.
pub const PRICE_PLACES: u32 = 5;

/// The lowest price an outcome order may name; a market sell goes at it.
pub const MIN_PRICE: Dec = Dec::lit::<PRICE_PLACES>(1);

/// The highest price an outcome order may name; a market buy goes at it.
pub const MAX_PRICE: Dec = Dec::lit::<PRICE_PLACES>(99_999);

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
        let encoding: u64 = digits.parse().ok()?;
        let side = match encoding % 10 {
            0 => OutcomeSide::Yes,
            1 => OutcomeSide::No,
            _ => return None,
        };
        Some(Coin { outcome: u32::try_from(encoding / 10).ok()?, side })
    }

    /// A limit order on this side, its price on the grid for `side` (a buy rounds down, a sell up)
    /// and its size cut to whole contracts.
    pub fn limit(self, side: Side, price: Dec, size: Dec, tif: Tif) -> Result<Order, OutcomeError> {
        let asset = self.asset().ok_or(OutcomeError::NoAsset(self))?;
        let price = outcome_price(price, side).ok_or(OutcomeError::Price(price))?;
        let size = outcome_size(size).ok_or(OutcomeError::Size(size))?;
        Ok(Order::limit(asset, side, price, size, tif))
    }

    /// A market order: `FrontendMarket` at the edge of the range, 0.99999 to buy and 0.00001 to
    /// sell, so it crosses whatever the book holds and never rests. Never `Gtc`.
    pub fn market(self, side: Side, size: Dec) -> Result<Order, OutcomeError> {
        let asset = self.asset().ok_or(OutcomeError::NoAsset(self))?;
        let size = outcome_size(size).ok_or(OutcomeError::Size(size))?;
        let bound = match side {
            Side::Buy => MAX_PRICE,
            Side::Sell => MIN_PRICE,
        };
        Ok(Order::market(asset, side, bound, size))
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

    /// Reads `#12100`, `+12100` or a bare `12100`, as a person types one. Venue data always carries
    /// its prefix: read it with [`Coin::parse`].
    #[must_use]
    pub fn lookup(text: &str) -> Option<Coin> {
        let text = text.trim();
        match text.starts_with(['#', '+']) {
            true => Coin::parse(text),
            false => Coin::parse(&format!("#{text}")),
        }
    }

    /// Whether `needle` appears in the encoding, a leading `#` or `+` aside.
    #[must_use]
    pub fn matches(self, needle: &str) -> bool {
        let needle = needle.strip_prefix(['#', '+']).unwrap_or(needle);
        self.encoding().to_string().contains(needle)
    }
}

/// `#12100`, as info and the websocket name it.
impl fmt::Display for Coin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.encoding())
    }
}

impl FromStr for Coin {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Coin::parse(text).ok_or_else(|| format!("`{text}` is not an outcome coin; expected `#12100`"))
    }
}

/// A price for `side` on the outcome grid, never against the trader; `None` outside 0.00001 to
/// 0.99999.
#[must_use]
pub fn outcome_price(price: Dec, side: Side) -> Option<Dec> {
    let kept = price.round(PRICE_PLACES, side.price_rounding());
    (MIN_PRICE..=MAX_PRICE).contains(&kept).then_some(kept)
}

/// A size cut toward zero to whole contracts; `None` when none are left.
#[must_use]
pub fn outcome_size(size: Dec) -> Option<Dec> {
    let kept = size.round(0, Round::Down);
    kept.is_positive().then_some(kept)
}

#[cfg(test)]
#[path = "tests/coin.rs"]
mod tests;
