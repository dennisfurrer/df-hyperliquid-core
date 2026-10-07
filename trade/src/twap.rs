//! `twapOrder` and `twapCancel`. Neither carries a builder in the SDK's
//! schemas: the slices a TWAP places cannot name one.

use hl_core::{Asset, Dec, L1Action, Map};

use crate::{error::TradeError, order::Side};

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
