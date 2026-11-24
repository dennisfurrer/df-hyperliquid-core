//! Where prices and sizes may sit on a perp or a spot pair: a price keeps at most
//! `6 - szDecimals` places on a perp and `8 - szDecimals` on spot, then five significant figures
//! unless it is whole; a size keeps `szDecimals`. A market order's bound is the reference moved by
//! the slippage. HIP-4 outcomes have their own grid ([`crate::markets::outcomes::px`]).

use crate::markets::Side;
use crate::trade::error::TradeError;
use crate::utils::{Dec, Round};

/// A price keeps at most this many significant figures, unless it is whole.
pub const SIGNIFICANT_FIGURES: u32 = 5;

/// Places a perp price may use before its size decimals are taken off.
pub const PERP_PRICE_PLACES: u32 = 6;

/// Places a spot price may use before its size decimals are taken off.
pub const SPOT_PRICE_PLACES: u32 = 8;

/// The venue refuses an order worth less than this, in the quote.
pub const MINIMUM_ORDER_VALUE: Dec = Dec::from_int(10);

/// The most slippage a market order is let take: half the price.
pub const MAX_SLIPPAGE: Dec = Dec::lit::<1>(5);

/// One asset's grid: its size decimals, and whether its prices take the perp
/// or the spot ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Grid {
    sz_decimals: u32,
    ceiling: u32,
}

impl Grid {
    /// A perp, main dex or HIP-3, by its `szDecimals` in `meta`.
    #[must_use]
    pub const fn perp(sz_decimals: u32) -> Grid {
        Grid { sz_decimals, ceiling: PERP_PRICE_PLACES }
    }

    /// A spot pair, by its base token's `szDecimals` in `spotMeta`.
    #[must_use]
    pub const fn spot(sz_decimals: u32) -> Grid {
        Grid { sz_decimals, ceiling: SPOT_PRICE_PLACES }
    }

    /// The asset's `szDecimals`: the places a size may use.
    #[must_use]
    pub const fn sz_decimals(self) -> u32 {
        self.sz_decimals
    }

    /// Places a price may use: six (perp) or eight (spot), less the size
    /// decimals.
    #[must_use]
    pub const fn price_places(self) -> u32 {
        self.ceiling.saturating_sub(self.sz_decimals)
    }

    /// `price` on the grid, rounded as asked, once: at the coarser of the
    /// places allowed and five significant figures, and never past the whole
    /// part (a whole price is always taken). Rounding to the places and then
    /// to the figures would round twice: to the nearest, `81800.45` would
    /// become `81801`, not `81800`. `None` when nothing positive is left.
    #[must_use]
    pub fn tick(self, price: Dec, round: Round) -> Option<Dec> {
        if !price.is_positive() {
            return None;
        }
        let places = self.price_places().min(price.sig_places(SIGNIFICANT_FIGURES));
        let kept = price.round(places, round);
        kept.is_positive().then_some(kept)
    }

    /// A limit price for `side`, never worse for the trader than asked: a buy
    /// rounds down and a sell up.
    #[must_use]
    pub fn price(self, price: Dec, side: Side) -> Option<Dec> {
        self.tick(price, side.price_rounding())
    }

    /// Whether the venue takes `price` exactly as written.
    #[must_use]
    pub fn is_on_grid(self, price: Dec) -> bool {
        self.tick(price, Round::Down) == Some(price)
    }

    /// A size cut toward zero to the size decimals. `None` when nothing is
    /// left.
    #[must_use]
    pub fn size(self, size: Dec) -> Option<Dec> {
        if !size.is_positive() {
            return None;
        }
        let kept = size.round(self.sz_decimals, Round::Down);
        kept.is_positive().then_some(kept)
    }

    /// The worst price a market order takes: `reference` moved by `slippage`
    /// (a fraction, 0.01 for 1%) on the side that costs more, then rounded
    /// back toward `reference`, so the bound is never looser than asked.
    pub fn market_bound(self, reference: Dec, slippage: Dec, side: Side) -> Result<Dec, TradeError> {
        if slippage.is_negative() || slippage > MAX_SLIPPAGE {
            return Err(TradeError::Slippage(slippage));
        }
        let factor = match side {
            Side::Buy => Dec::ONE.checked_add(slippage),
            Side::Sell => Dec::ONE.checked_sub(slippage),
        };
        factor
            .and_then(|factor| reference.checked_mul(factor))
            .and_then(|moved| self.price(moved, side))
            .ok_or(TradeError::NoPrice(reference))
    }

    /// The smallest size worth [`MINIMUM_ORDER_VALUE`] at `price`, rounded up
    /// to the size decimals.
    #[must_use]
    pub fn least_size(self, price: Dec) -> Option<Dec> {
        let exact = MINIMUM_ORDER_VALUE.checked_div(price, 12, Round::Up)?;
        let size = exact.round(self.sz_decimals, Round::Up);
        size.is_positive().then_some(size)
    }

    /// Whether `size` at `price` is worth the venue's minimum.
    #[must_use]
    pub fn meets_minimum(price: Dec, size: Dec) -> bool {
        price.checked_mul(size).is_some_and(|worth| worth >= MINIMUM_ORDER_VALUE)
    }
}

#[cfg(test)]
#[path = "tests/rounding.rs"]
mod tests;
