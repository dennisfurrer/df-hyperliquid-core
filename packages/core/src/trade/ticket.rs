//! What every order ticket shares: the market's rules ([`Market`]), how much ([`Amount`]), why
//! nothing is sent ([`Refusal`]), and the checks each order type runs. A price never rounds
//! against the trader and a size rounds toward zero.

use crate::trade::builder::BuilderSetting;
use crate::trade::error::TradeError;
use crate::trade::orders::scale::{LEAST_LEVELS, MOST_LEVELS};
use crate::trade::orders::{Order, PlaceOrder, Tif, Tpsl};
use crate::utils::rounding::Grid;
use crate::utils::{Asset, AssetKind, Dec, Round};

/// What the ticket knows of one market, from the meta the caller read: its asset id, its grid,
/// and for a perp its leverage ceiling and whether it takes cross margin. HIP-4 outcomes have
/// orders of their own in [`crate::markets::outcomes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Market {
    asset: Asset,
    grid: Grid,
    max_leverage: Option<u32>,
    allows_cross: bool,
}

impl Market {
    /// A perp, main dex or HIP-3: its `szDecimals` and `maxLeverage` in
    /// `meta`, and whether it takes cross (not `onlyIsolated`, and no
    /// `marginMode` that forbids it). `None` for an id that names no perp.
    #[must_use]
    pub const fn perp(asset: Asset, sz_decimals: u32, max_leverage: u32, allows_cross: bool) -> Option<Market> {
        match asset.kind() {
            AssetKind::Perp | AssetKind::Hip3Perp => {
                Some(Market { asset, grid: Grid::perp(sz_decimals), max_leverage: Some(max_leverage), allows_cross })
            }
            AssetKind::Spot | AssetKind::Outcome => None,
        }
    }

    /// A spot pair, by its base token's `szDecimals` in `spotMeta`. `None`
    /// for an id that names no pair.
    #[must_use]
    pub const fn spot(asset: Asset, sz_decimals: u32) -> Option<Market> {
        match asset.kind() {
            AssetKind::Spot => {
                Some(Market { asset, grid: Grid::spot(sz_decimals), max_leverage: None, allows_cross: false })
            }
            AssetKind::Perp | AssetKind::Hip3Perp | AssetKind::Outcome => None,
        }
    }

    #[must_use]
    pub const fn asset(&self) -> Asset {
        self.asset
    }

    #[must_use]
    pub const fn grid(&self) -> Grid {
        self.grid
    }

    /// `None` on spot, which takes no leverage.
    #[must_use]
    pub const fn max_leverage(&self) -> Option<u32> {
        self.max_leverage
    }

    #[must_use]
    pub const fn allows_cross(&self) -> bool {
        self.allows_cross
    }

    /// Spot has balances, not positions: nothing that closes, protects or
    /// funds one.
    #[must_use]
    pub const fn is_spot(&self) -> bool {
        self.max_leverage.is_none()
    }
}

/// How much: a size in the base, or a notional in the quote that is cut to
/// a size at the order's price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    Size(Dec),
    Notional(Dec),
}

impl Amount {
    /// The size on `grid` at `price`, cut toward zero.
    pub(crate) fn size_on(self, grid: Grid, price: Dec) -> Result<Dec, Refusal> {
        let raw = match self {
            Amount::Size(size) => positive(size, "a size")?,
            Amount::Notional(notional) => positive(notional, "a notional")?
                .checked_div(price, 12, Round::Down)
                .ok_or(Refusal::Trade(TradeError::NoPrice(price)))?,
        };
        grid.size(raw).ok_or(Refusal::Trade(TradeError::NoSize(raw)))
    }
}

/// Why the ticket sends nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error(transparent)]
    Trade(#[from] TradeError),
    #[error("{what} above zero is needed, not {value}")]
    NotPositive { what: &'static str, value: Dec },
    #[error("{size} at {price} is worth {}, under the venue's minimum of {minimum}", worth(.size, .price))]
    UnderMinimum { size: Dec, price: Dec, minimum: Dec },
    #[error("a limit is never FrontendMarket: a market order is Price::Market, bounded from the mid")]
    MarketTif,
    #[error("a scale rests on the book, so Gtc or Alo, not {}", .0.wire())]
    ScaleTif(Tif),
    #[error("a scale has {LEAST_LEVELS} to {MOST_LEVELS} levels, not {0}")]
    Levels(u32),
    #[error("a modify that does not always place is Alo or Gtc, not {}", .0.wire())]
    ModifyTif(Tif),
    #[error("spot takes no {0}")]
    Spot(&'static str),
    #[error("a notional needs a reference price to be sized at")]
    NoReference,
    #[error("a {} at {trigger} is on the wrong side of an entry at {entry}", tpsl_words(.tpsl))]
    ExitSide { tpsl: Tpsl, trigger: Dec, entry: Dec },
    #[error("there is no position here")]
    NoPosition,
    #[error("closing {size} is more than the position of {position}")]
    CloseTooLarge { size: Dec, position: Dec },
    #[error("leverage is a whole number from 1 to {most}, not {asked}")]
    Leverage { asked: u32, most: u32 },
    #[error("this market is isolated only")]
    IsolatedOnly,
    #[error("this position is cross; margin moves on isolated ones")]
    CrossPosition,
    #[error("order {0} is a trigger: change its trigger instead")]
    NotALimit(u64),
    #[error("order {0} is not a take profit or stop loss")]
    NotATrigger(u64),
    #[error("order {oid} is a {kind} trigger, which this build cannot modify")]
    UnknownTrigger { oid: u64, kind: String },
    #[error("order {0} has a size of its own; only a position's take profit or stop loss covers the whole position")]
    NotWholePosition(u64),
}

fn worth(size: &Dec, price: &Dec) -> String {
    size.checked_mul(*price).map_or_else(|| "too much to count".to_owned(), Dec::to_wire)
}

const fn tpsl_words(tpsl: &Tpsl) -> &'static str {
    match tpsl {
        Tpsl::TakeProfit => "take profit",
        Tpsl::StopLoss => "stop loss",
    }
}

pub(crate) fn positive(value: Dec, what: &'static str) -> Result<Dec, Refusal> {
    match value.is_positive() {
        true => Ok(value),
        false => Err(Refusal::NotPositive { what, value }),
    }
}

pub(crate) fn at_least(size: Dec, price: Dec, minimum: Dec) -> Result<(), Refusal> {
    match price.checked_mul(size).is_some_and(|worth| worth >= minimum) {
        true => Ok(()),
        false => Err(Refusal::UnderMinimum { size, price, minimum }),
    }
}

/// `orders` through the builder, held to their strictest cap.
pub(crate) fn routed(place: PlaceOrder, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    let Some(setting) = builder else { return Ok(place) };
    match setting.on_assets(place.orders().iter().map(Order::asset)).map_err(TradeError::from)? {
        Some(builder) => Ok(place.through(builder)),
        None => Ok(place),
    }
}

#[cfg(test)]
#[path = "tests/ticket.rs"]
mod tests;
