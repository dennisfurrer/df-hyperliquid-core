//! An account's histories on the socket: fills, funding payments and ledger rows, each opening
//! with the past (`is_snapshot`) and then each new row as it happens.

use crate::account::history::{Fill, LedgerUpdate};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `userFills`: the last fills first (`is_snapshot`), then each new one as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFills {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub fills: Vec<Fill>,
}

/// One funding payment: on `userFundings`, and as a `userEvents` event on the hour.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Funding {
    /// Milliseconds.
    pub time: u64,
    pub coin: String,
    /// Paid out (negative) or received.
    pub usdc: Dec,
    /// The position it was paid on, signed.
    pub szi: Dec,
    pub funding_rate: Dec,
    #[serde(default)]
    pub n_samples: Option<u64>,
}

/// `userFundings`: past payments first (`is_snapshot`), then one an hour per position.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFundings {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub fundings: Vec<Funding>,
}

/// `userNonFundingLedgerUpdates`: deposits, withdrawals, transfers, sends, vault moves and
/// liquidations; past ones first (`is_snapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "nonFundingLedgerUpdates")]
    pub updates: Vec<LedgerUpdate>,
}

#[cfg(test)]
#[path = "tests/history.rs"]
mod tests;
