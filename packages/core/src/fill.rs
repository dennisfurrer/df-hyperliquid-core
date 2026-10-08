//! An account's fills, the same shape whether `userFills`, `userFillsByTime` and
//! `userTwapSliceFills` answer them or `userFills`, `userEvents` and the TWAP feeds send them.
//! Perp, spot and outcome fills come in the same answer.

use serde::Deserialize;

use crate::{address::Address, dec::Dec, side::Side};

/// One fill. `fee` is everything the fill cost, a builder's share included, in `fee_token`:
/// USDC, or on a spot buy the token bought.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    /// `BTC`, `xyz:XYZ100`, `@107`, `#14730`.
    pub coin: String,
    pub px: Dec,
    pub sz: Dec,
    /// The account's side.
    pub side: Side,
    /// Milliseconds.
    pub time: u64,
    /// The account's position in the coin before this fill, signed.
    pub start_position: Dec,
    /// For display: `Open Long`, `Close Short`, `Buy`, `Spot Dust Conversion`, `Settlement`...
    pub dir: String,
    pub closed_pnl: Dec,
    /// The L1 transaction hash; all zeros for a TWAP slice.
    pub hash: String,
    pub oid: u64,
    /// Took liquidity.
    pub crossed: bool,
    /// Negative is a rebate. Includes `builder_fee`.
    pub fee: Dec,
    pub tid: u64,
    /// The order's client id, as written: every recorded one is `0x` and 32 hex digits, but a
    /// fill is read whole whatever its order was tagged with.
    #[serde(default)]
    pub cloid: Option<String>,
    pub fee_token: String,
    /// What the order's builder took; the venue leaves it out when nothing.
    #[serde(default)]
    pub builder_fee: Option<Dec>,
    /// Only on a fill that was part of a liquidation.
    #[serde(default)]
    pub liquidation: Option<FillLiquidation>,
    #[serde(default)]
    pub twap_id: Option<u64>,
}

/// On a fill that was part of a liquidation, on either side of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillLiquidation {
    #[serde(default)]
    pub liquidated_user: Option<Address>,
    pub mark_px: Dec,
    /// `market` or `backstop`.
    pub method: String,
}

/// A fill of one of a TWAP's slices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFill {
    pub fill: Fill,
    pub twap_id: u64,
}

#[cfg(test)]
#[path = "tests/fill.rs"]
mod tests;
