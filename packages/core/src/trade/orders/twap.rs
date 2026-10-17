//! TWAP: the venue slices an order into one every thirty seconds for 5 minutes to a day.
//! `twapOrder` and `twapCancel` carry no builder: the slices cannot name one.

use crate::trade::error::TradeError;
use crate::trade::orders::Side;
use crate::trade::ticket::{Amount, Market, Refusal, at_least, positive};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Asset, Dec};

/// The least a TWAP may total, in the quote: the venue's order types page.
pub const MINIMUM_TWAP_VALUE: Dec = Dec::from_int(100);

/// `twapOrder`: the venue slices `size` into an order every thirty seconds
/// for `minutes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Twap {
    asset: Asset,
    side: Side,
    size: Dec,
    reduce_only: bool,
    minutes: u32,
    randomize: bool,
}

impl Twap {
    pub const MIN_MINUTES: u32 = 5;
    pub const MAX_MINUTES: u32 = 1440;

    /// `size` already cut to the asset's size decimals.
    pub fn new(asset: Asset, side: Side, size: Dec, minutes: u32) -> Result<Twap, TradeError> {
        if !(Twap::MIN_MINUTES..=Twap::MAX_MINUTES).contains(&minutes) {
            return Err(TradeError::TwapMinutes(minutes));
        }
        Ok(Twap { asset, side, size, reduce_only: false, minutes, randomize: false })
    }

    #[must_use]
    pub const fn reduce_only(mut self) -> Twap {
        self.reduce_only = true;
        self
    }

    /// Lets the venue vary each slice's size.
    #[must_use]
    pub const fn randomized(mut self) -> Twap {
        self.randomize = true;
        self
    }
}

impl L1Action for Twap {
    fn action(&self) -> Map {
        Map::new().put("type", "twapOrder").put(
            "twap",
            Map::new()
                .put("a", self.asset)
                .put("b", self.side.is_buy())
                .put("s", self.size.to_wire())
                .put("r", self.reduce_only)
                .put("m", self.minutes)
                .put("t", self.randomize),
        )
    }
}

/// `twapCancel`, flat: `{type, a, t}`, `t` the id the venue gave the TWAP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwapCancel {
    asset: Asset,
    twap: u64,
}

impl TwapCancel {
    #[must_use]
    pub const fn new(asset: Asset, twap: u64) -> TwapCancel {
        TwapCancel { asset, twap }
    }
}

impl L1Action for TwapCancel {
    fn action(&self) -> Map {
        Map::new().put("type", "twapCancel").put("a", self.asset).put("t", self.twap)
    }
}

/// A TWAP: the venue slices it into an order every thirty seconds for
/// `minutes`. `reference` prices a notional and the venue's minimum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwapIntent {
    pub side: Side,
    pub amount: Amount,
    pub minutes: u32,
    pub randomize: bool,
    pub reduce_only: bool,
    pub reference: Option<Dec>,
}

impl TwapIntent {
    #[must_use]
    pub const fn new(side: Side, amount: Amount, minutes: u32) -> TwapIntent {
        TwapIntent { side, amount, minutes, randomize: false, reduce_only: false, reference: None }
    }

    /// Priced at `reference`, the mid or else the mark.
    #[must_use]
    pub const fn at(mut self, reference: Dec) -> TwapIntent {
        self.reference = Some(reference);
        self
    }
}

/// A `twapOrder`. It carries no builder: the venue gives it no field for one.
pub fn twap(market: &Market, intent: &TwapIntent) -> Result<Twap, Refusal> {
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    let size = match (intent.amount, intent.reference) {
        (Amount::Notional(_), None) => return Err(Refusal::NoReference),
        (amount, Some(reference)) => amount.size_on(market.grid(), positive(reference, "a reference price")?)?,
        (Amount::Size(size), None) => {
            let size = positive(size, "a size")?;
            market.grid().size(size).ok_or(Refusal::Trade(TradeError::NoSize(size)))?
        }
    };
    if let Some(reference) = intent.reference {
        at_least(size, reference, MINIMUM_TWAP_VALUE)?;
    }
    let twap = Twap::new(market.asset(), intent.side, size, intent.minutes)?;
    let twap = match intent.reduce_only {
        true => twap.reduce_only(),
        false => twap,
    };
    Ok(match intent.randomize {
        true => twap.randomized(),
        false => twap,
    })
}

#[cfg(test)]
#[path = "tests/twap.rs"]
mod tests;
