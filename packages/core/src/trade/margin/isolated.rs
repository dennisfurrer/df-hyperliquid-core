//! `updateIsolatedMargin`: USDC into or out of an isolated position.

use crate::trade::error::TradeError;
use crate::trade::margin::MarginMode;
use crate::trade::orders::Side;
use crate::trade::ticket::{Market, Refusal};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Asset, BadUnits, Dec, Round};

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

    /// The perp's asset id.
    #[must_use]
    pub const fn asset(&self) -> Asset {
        self.asset
    }

    /// `ntli`: the USDC amount in millionths (six places), positive to add margin and negative to remove it.
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

/// More USDC into an isolated position, long (`Side::Buy`) or short.
pub fn add_margin(asset: Asset, position: Side, usdc: Dec) -> Result<UpdateIsolatedMargin, TradeError> {
    match usdc.is_positive() {
        true => UpdateIsolatedMargin::new(asset, position, usdc),
        false => Err(TradeError::Margin(BadUnits::NotPositive(usdc))),
    }
}

/// USDC out of an isolated position: sent as a negative `ntli`.
pub fn remove_margin(asset: Asset, position: Side, usdc: Dec) -> Result<UpdateIsolatedMargin, TradeError> {
    match usdc.is_positive() {
        true => UpdateIsolatedMargin::new(asset, position, usdc.negate()),
        false => Err(TradeError::Margin(BadUnits::NotPositive(usdc))),
    }
}

/// `updateIsolatedMargin` on an isolated `position` (signed `szi`): `usdc`
/// above zero adds margin, below zero takes it out.
pub fn margin(market: &Market, position: Dec, mode: MarginMode, usdc: Dec) -> Result<UpdateIsolatedMargin, Refusal> {
    if market.is_spot() {
        return Err(Refusal::Spot("margin"));
    }
    if position.is_zero() {
        return Err(Refusal::NoPosition);
    }
    if mode == MarginMode::Cross {
        return Err(Refusal::CrossPosition);
    }
    let side = match position.is_positive() {
        true => Side::Buy,
        false => Side::Sell,
    };
    Ok(UpdateIsolatedMargin::new(market.asset(), side, usdc)?)
}

#[cfg(test)]
#[path = "tests/isolated.rs"]
mod tests;
