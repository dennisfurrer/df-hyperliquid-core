//! `userNonFundingLedgerUpdates`: every movement of an account's money other than funding. A
//! `delta` type this build does not know reads as [`Delta::Other`], kept as written.

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Json};

/// `{"type":"userNonFundingLedgerUpdates","user":..,"startTime":..,"endTime"?:..}`,
/// times in milliseconds.
#[must_use]
pub fn ledger_request(user: Address, start_time: u64, end_time: Option<u64>) -> String {
    let mut fields = vec![("user", Json::String(user.to_hex())), ("startTime", Json::Number(start_time.to_string()))];
    if let Some(end_time) = end_time {
        fields.push(("endTime", Json::Number(end_time.to_string())));
    }
    info_request("userNonFundingLedgerUpdates", &fields)
}

/// The row a send signed with `nonce` made. Only `send` and `spotTransfer` rows carry their
/// action's nonce.
#[must_use]
pub fn find(rows: &[LedgerUpdate], nonce: u64) -> Option<&LedgerUpdate> {
    rows.iter().find(|row| {
        matches!(row.delta, Delta::Send { nonce: Some(said), .. } | Delta::SpotTransfer { nonce: Some(said), .. } if said == nonce)
    })
}

/// The rows as the venue sends them, oldest first.
pub async fn ledger(
    transport: &impl Transport,
    user: Address,
    start_time: u64,
    end_time: Option<u64>,
) -> Result<Vec<LedgerUpdate>, ApiError> {
    info(transport, ledger_request(user, start_time, end_time)).await
}

/// One row and what it did, in the shapes the socket sends too.
pub use crate::account::ledger::{Delta, LedgerUpdate};

#[cfg(test)]
#[path = "tests/ledger.rs"]
mod tests;
