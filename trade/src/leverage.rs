//! `updateLeverage` and `updateIsolatedMargin`: perps only, main dex or HIP-3.

use hl_core::{Asset, BadUnits, Dec, L1Action, Map, Round};

use crate::{error::TradeError, order::Side};

/// How a position is margined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarginMode {
    /// Shares the account's perp balance.
    Cross,
    /// Margined on its own.
    Isolated,
}

/// `updateLeverage`: `{type, asset, isCross, leverage}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateLeverage {
    asset: Asset,
    mode: MarginMode,
    leverage: u32,
}

impl UpdateLeverage {
    /// A whole number, at least 1. What the asset allows (its
    /// `maxLeverage`, whether it takes cross) is for the market crate to say.
    pub fn new(asset: Asset, mode: MarginMode, leverage: u32) -> Result<UpdateLeverage, TradeError> {
        match leverage {
            0 => Err(TradeError::NoLeverage),
            leverage => Ok(UpdateLeverage { asset, mode, leverage }),
        }
    }

    #[must_use]
    pub const fn leverage(&self) -> u32 {
        self.leverage
    }

    #[must_use]
    pub const fn mode(&self) -> MarginMode {
        self.mode
    }
}

impl L1Action for UpdateLeverage {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "updateLeverage")
            .put("asset", self.asset)
            .put("isCross", matches!(self.mode, MarginMode::Cross))
            .put("leverage", self.leverage)
    }
}

/// `updateIsolatedMargin`: `{type, asset, isBuy, ntli}`. `ntli` is USDC in
/// millionths, negative to take margin out; `isBuy` names the position's
/// side, long or short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateIsolatedMargin {
    asset: Asset,
    position: Side,
    ntli: i64,
}

impl UpdateIsolatedMargin {
    /// `usdc` positive to add margin and negative to remove it, at most six
    /// places: refused rather than rounded when finer.
    pub fn new(asset: Asset, position: Side, usdc: Dec) -> Result<UpdateIsolatedMargin, TradeError> {
        let bad = |error: BadUnits| TradeError::Margin(error);
        if usdc.places() > 6 {
            return Err(bad(BadUnits::TooFine { value: usdc, places: 6 }));
        }
        let ntli = usdc.units_at(6, Round::Down).and_then(|units| i64::try_from(units).ok());
        match ntli {
            Some(0) => Err(bad(BadUnits::NotPositive(usdc))),
            Some(ntli) => Ok(UpdateIsolatedMargin { asset, position, ntli }),
            None => Err(bad(BadUnits::TooLarge(usdc))),
        }
    }

    #[must_use]
    pub const fn ntli(&self) -> i64 {
        self.ntli
    }
}

impl L1Action for UpdateIsolatedMargin {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "updateIsolatedMargin")
            .put("asset", self.asset)
            .put("isBuy", self.position.is_buy())
            .put("ntli", self.ntli)
    }
}
