//! An account's TWAPs on the socket: the ones running on a dex, each slice as it fills, and each
//! one starting, finishing, being stopped or failing.

use crate::account::TwapSliceFill;
use crate::markets::Side;
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `twapStates`: the account's running TWAPs on one dex, by id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwapStates {
    pub dex: String,
    pub user: Address,
    pub states: Vec<(u64, TwapState)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapState {
    pub coin: String,
    pub user: Address,
    pub side: Side,
    pub sz: Dec,
    pub executed_sz: Dec,
    pub executed_ntl: Dec,
    pub minutes: u32,
    pub reduce_only: bool,
    pub randomize: bool,
    /// When it started, milliseconds.
    pub timestamp: u64,
}

/// `userTwapSliceFills`: past slices first (`is_snapshot`), then each as it fills.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFills {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "twapSliceFills")]
    pub fills: Vec<TwapSliceFill>,
}

/// A TWAP starting, finishing, being stopped or failing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapEvent {
    /// Seconds, not milliseconds: the venue writes this one so.
    pub time: u64,
    pub state: TwapState,
    pub status: TwapStatus,
    #[serde(default)]
    pub twap_id: Option<u64>,
}

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
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub history: Vec<TwapEvent>,
}

#[cfg(test)]
#[path = "tests/twap.rs"]
mod tests;
