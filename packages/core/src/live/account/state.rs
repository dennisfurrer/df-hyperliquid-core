//! An account's clearinghouse frames: one dex's, every dex's at once, and its spot balances, each
//! in the shape the `/info` read answers. What it may trade on one perp (`activeAssetData`) comes
//! as [`crate::account::ActiveAssetData`].

use crate::account::ClearinghouseState;
use crate::utils::Address;
use serde::Deserialize;

/// `clearinghouseState`: one dex's clearinghouse (`dex` empty for the first).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexState {
    pub dex: String,
    pub user: Address,
    #[serde(rename = "clearinghouseState")]
    pub state: ClearinghouseState,
}

/// `allDexsClearinghouseState`: every dex's clearinghouse, the first named `""`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllDexsState {
    pub user: Address,
    #[serde(rename = "clearinghouseStates")]
    pub states: Vec<(String, ClearinghouseState)>,
}

/// `spotState`: spot balances, outcome sides among them (`+14720`, with no token).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotState {
    pub user: Address,
    /// The shape `spotClearinghouseState` answers.
    #[serde(rename = "spotState")]
    pub state: crate::account::SpotState,
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
