//! Asset contexts: what a market is doing now. [`PerpCtx`] for a perp on any dex, [`SpotCtx`] for a
//! spot pair or an outcome side, each the same shape from `/info` and from the socket.

use serde::Deserialize;

use crate::utils::dec::{Dec, Round};

/// What a perp is doing now. The venue writes `midPx`, `premium` and `impactPxs` as null on a
/// market with no book.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpCtx {
    /// The mark price (`markPx`).
    pub mark_px: Dec,
    /// The mid price (`midPx`); `None` on a market with no book.
    #[serde(default)]
    pub mid_px: Option<Dec>,
    /// The oracle price (`oraclePx`).
    pub oracle_px: Dec,
    /// The mark price a day ago (`prevDayPx`).
    pub prev_day_px: Dec,
    /// Hourly, as a fraction of the position's value.
    pub funding: Dec,
    /// In the base asset.
    pub open_interest: Dec,
    /// Quote volume over the last day.
    pub day_ntl_vlm: Dec,
    /// Base volume over the last day.
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    /// The premium over the oracle price that funding is computed from, as a fraction; `None` on a
    /// market with no book.
    #[serde(default)]
    pub premium: Option<Dec>,
    /// The impact bid and ask prices (`impactPxs`); `None` on a market with no book.
    #[serde(default)]
    pub impact_pxs: Option<Vec<Dec>>,
}

impl PerpCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }

    /// The mark's change over the day in percent, to two places. `None` without a previous day's
    /// price.
    #[must_use]
    pub fn day_change(&self) -> Option<Dec> {
        let then = Some(self.prev_day_px).filter(|then| then.is_positive())?;
        self.mark_px.checked_sub(then)?.checked_mul(Dec::from_int(100))?.checked_div(then, 2, Round::Nearest)
    }

    /// The open interest at the mark, in the quote.
    #[must_use]
    pub fn open_interest_value(&self) -> Option<Dec> {
        self.open_interest.checked_mul(self.mark_px)
    }
}

/// What a spot pair or an outcome side (`#<encoding>`) is doing now. An outcome side's open
/// interest is its circulating supply.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotCtx {
    /// The book name: `PURR/USDC`, `@107` or `#14730`.
    pub coin: String,
    /// The mark price (`markPx`).
    pub mark_px: Dec,
    /// The mid price (`midPx`); `None` on a market with no book.
    #[serde(default)]
    pub mid_px: Option<Dec>,
    /// The mark price a day ago (`prevDayPx`).
    pub prev_day_px: Dec,
    /// Quote volume over the last day.
    pub day_ntl_vlm: Dec,
    /// Base volume over the last day.
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    /// The circulating supply (`circulatingSupply`), in the base token; an outcome side's open interest.
    pub circulating_supply: Dec,
    /// The total supply (`totalSupply`), in the base token.
    #[serde(default)]
    pub total_supply: Option<Dec>,
}

impl SpotCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }
}

#[cfg(test)]
#[path = "tests/context.rs"]
mod tests;
