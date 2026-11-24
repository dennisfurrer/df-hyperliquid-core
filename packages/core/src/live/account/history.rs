//! An account's histories on the socket: fills, funding payments and ledger rows, each opening
//! with the past (`is_snapshot`) and then each new row as it happens.

use crate::account::history::{Fill, LedgerUpdate};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `userFills`: the last fills first (`is_snapshot`), then each new one as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFills {
    /// The account the fills are for.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past fills; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// The fills in this frame.
    pub fills: Vec<Fill>,
}

/// One funding payment: on `userFundings`, and as a `userEvents` event on the hour.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Funding {
    /// Milliseconds.
    pub time: u64,
    /// The perp the payment was on.
    pub coin: String,
    /// Paid out (negative) or received.
    pub usdc: Dec,
    /// The position it was paid on, signed.
    pub szi: Dec,
    /// `fundingRate`: the rate the payment was charged at, per hour, as a fraction.
    pub funding_rate: Dec,
    /// `nSamples`: how many premium samples the rate came from; `None` when not sent.
    #[serde(default)]
    pub n_samples: Option<u64>,
}

/// `userFundings`: past payments first (`is_snapshot`), then one an hour per position.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFundings {
    /// The account the payments are for.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past payments; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// The payments in this frame.
    pub fundings: Vec<Funding>,
}

/// `userNonFundingLedgerUpdates`: deposits, withdrawals, transfers, sends, vault moves and
/// liquidations; past ones first (`is_snapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    /// The account the rows are for.
    pub user: Address,
    /// `isSnapshot`: true on the first frame, which carries past rows; false when not sent.
    #[serde(default)]
    pub is_snapshot: bool,
    /// `nonFundingLedgerUpdates`: the ledger rows in this frame.
    #[serde(rename = "nonFundingLedgerUpdates")]
    pub updates: Vec<LedgerUpdate>,
}

#[cfg(test)]
#[path = "tests/history.rs"]
mod tests;
