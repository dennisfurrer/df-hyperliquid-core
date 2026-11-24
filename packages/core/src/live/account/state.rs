//! An account's clearinghouse frames: one dex's, every dex's at once, and its spot balances, each
//! in the shape the `/info` read answers. What it may trade on one perp (`activeAssetData`) comes
//! as [`crate::account::ActiveAssetData`].

use crate::account::ClearinghouseState;
use crate::utils::Address;
use serde::Deserialize;

/// `clearinghouseState`: one dex's clearinghouse (`dex` empty for the first).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DexState {
    /// The dex, `""` for the first perp dex.
    pub dex: String,
    /// The account the state belongs to.
    pub user: Address,
    /// `clearinghouseState`: the dex's clearinghouse, as the `/info` read answers it.
    #[serde(rename = "clearinghouseState")]
    pub state: ClearinghouseState,
}

/// `allDexsClearinghouseState`: every dex's clearinghouse, the first named `""`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AllDexsState {
    /// The account the states belong to.
    pub user: Address,
    /// `clearinghouseStates`: each dex's name and clearinghouse, the first named `""`.
    #[serde(rename = "clearinghouseStates")]
    pub states: Vec<(String, ClearinghouseState)>,
}

/// `spotState`: spot balances, outcome sides among them (`+14720`, with no token).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SpotState {
    /// The account the balances belong to.
    pub user: Address,
    /// The shape `spotClearinghouseState` answers.
    #[serde(rename = "spotState")]
    pub state: crate::account::SpotState,
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
