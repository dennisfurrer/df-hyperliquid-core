//! What a perp accepts, in one place: price grid, size step, minimum order value, leverage and
//! margin modes; and what a coin tracks.

use crate::markets::perps::meta::PerpAsset;
use crate::trade::MarginMode;
use crate::utils::Dec;
use crate::utils::{Grid, MINIMUM_ORDER_VALUE, PERP_PRICE_PLACES};

const BOTH: [MarginMode; 2] = [MarginMode::Cross, MarginMode::Isolated];
const ISOLATED: [MarginMode; 1] = [MarginMode::Isolated];

/// Units of the underlying a `k` perp counts in.
const THOUSAND: Dec = Dec::from_int(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// Prices at five significant figures and at most `6 - szDecimals` places; sizes on
    /// `szDecimals`.
    pub grid: Grid,
    /// The smallest size: one at the last of `szDecimals`.
    pub size_step: Dec,
    /// The least an order may be worth, in the quote.
    pub min_value: Dec,
    pub max_leverage: u32,
    /// Set by `onlyIsolated`, `strictIsolated` or `noCross`.
    pub isolated_only: bool,
}

impl Rules {
    /// `None` past six size decimals, which would leave a price no place.
    #[must_use]
    pub fn of(perp: &PerpAsset) -> Option<Rules> {
        PERP_PRICE_PLACES.checked_sub(perp.sz_decimals)?;
        Some(Rules {
            grid: perp.grid(),
            size_step: Dec::new(1, perp.sz_decimals)?,
            min_value: MINIMUM_ORDER_VALUE,
            max_leverage: perp.max_leverage,
            isolated_only: !perp.allows_cross(),
        })
    }

    /// The margin modes a position may take.
    #[must_use]
    pub const fn margin_modes(&self) -> &'static [MarginMode] {
        match self.isolated_only {
            true => &ISOLATED,
            false => &BOTH,
        }
    }
}

/// What a coin tracks and how many units of it one contract is: `kPEPE` is a thousand PEPE. A
/// HIP-3 coin's `dex:` is left off: `xyz:TSLA` tracks TSLA.
#[must_use]
pub fn underlying(coin: &str) -> (&str, Dec) {
    let bare = coin.split_once(':').map_or(coin, |(_, symbol)| symbol);
    match bare.strip_prefix('k') {
        Some(rest) if rest.starts_with(|first: char| first.is_ascii_uppercase()) => (rest, THOUSAND),
        _ => (bare, Dec::ONE),
    }
}
