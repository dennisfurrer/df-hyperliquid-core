//! Asset contexts: what a market is doing now. A perp's is the same shape on the main dex and on
//! a HIP-3 dex, whether `metaAndAssetCtxs` answers it or `activeAssetCtx`, `assetCtxs` and
//! `allDexsAssetCtxs` send it; a spot pair's or an outcome side's is the same from
//! `spotMetaAndAssetCtxs` and from `activeSpotAssetCtx` and `spotAssetCtxs`.

use serde::Deserialize;

use crate::dec::{Dec, Round};

/// What a perp is doing now. The venue writes `midPx`, `premium` and `impactPxs` as null on a
/// market with no book.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpCtx {
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub oracle_px: Dec,
    pub prev_day_px: Dec,
    /// Hourly, as a fraction of the position's value.
    pub funding: Dec,
    /// In the base asset.
    pub open_interest: Dec,
    /// The quote traded over the last day.
    pub day_ntl_vlm: Dec,
    /// The base traded over the last day.
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    #[serde(default)]
    pub premium: Option<Dec>,
    #[serde(default)]
    pub impact_pxs: Option<Vec<Dec>>,
}

impl PerpCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }

    /// The mark's change over the day, in percent to two places, as hip4-tui shows it. `None`
    /// without a previous day's price.
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
    /// The pair's or outcome side's book name.
    pub coin: String,
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub prev_day_px: Dec,
    /// The quote traded over the last day.
    pub day_ntl_vlm: Dec,
    /// The base traded over the last day.
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    pub circulating_supply: Dec,
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
