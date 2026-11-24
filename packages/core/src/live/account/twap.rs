//! An account's TWAPs on the socket: the ones running on a dex, each slice as it fills, and each
//! one starting, finishing, being stopped or failing.

use crate::account::TwapSliceFill;
use crate::markets::Side;
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `twapStates`: the account's running TWAPs on one dex, by id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwapStates {
    /// The dex, `""` for the first perp dex.
    pub dex: String,
    /// The account the TWAPs belong to.
    pub user: Address,
    /// Each running TWAP's id and state.
    pub states: Vec<(u64, TwapState)>,
}

/// One TWAP's settings and how much of it has traded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapState {
    /// The coin it trades.
    pub coin: String,
    /// The account it trades for.
    pub user: Address,
    /// Buy or sell.
    pub side: Side,
    /// The whole size to trade, in the coin's units.
    pub sz: Dec,
    /// `executedSz`: the size traded so far, in the coin's units.
    pub executed_sz: Dec,
    /// `executedNtl`: the notional traded so far, USDC.
    pub executed_ntl: Dec,
    /// How long it runs, in minutes.
    pub minutes: u32,
    /// `reduceOnly`: whether its slices may only reduce a position.
    pub reduce_only: bool,
    /// Whether the venue randomizes the slices.
    pub randomize: bool,
    /// When it started, milliseconds.
    pub timestamp: u64,
}

/// `userTwapSliceFills`: past slices first (`is_snapshot`), then each as it fills.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFills {
    /// The account the fills are for.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past slices; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// `twapSliceFills`: the slice fills in this frame.
    #[serde(rename = "twapSliceFills")]
    pub fills: Vec<TwapSliceFill>,
}

/// A TWAP starting, finishing, being stopped or failing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapEvent {
    /// Seconds, not milliseconds: the venue writes this one so.
    pub time: u64,
    /// The TWAP's settings and progress at the time.
    pub state: TwapState,
    /// What happened to it.
    pub status: TwapStatus,
    /// `twapId`: the TWAP's id; `None` when not sent.
    #[serde(default)]
    pub twap_id: Option<u64>,
}

/// The status in a [`TwapEvent`], and why on an error.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwapStatus {
    /// `activated`, `finished`, `terminated` or `error`.
    pub status: String,
    /// Why, on an error.
    #[serde(default)]
    pub description: Option<String>,
}

/// `userTwapHistory`: past TWAP events first (`is_snapshot`), then each as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapHistory {
    /// The account the events are for.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past events; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// The TWAP events in this frame.
    pub history: Vec<TwapEvent>,
}

#[cfg(test)]
#[path = "tests/twap.rs"]
mod tests;
