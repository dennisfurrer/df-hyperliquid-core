//! `userNonFundingLedgerUpdates`: every movement of an account's money other
//! than funding. Each row's `delta` is tagged by `type`; a type this build
//! does not know reads as [`Delta::Other`], kept as written, rather than
//! failing the list.

use hl_core::{Address, ApiError, Json, Transport, info, info_request};

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

/// The row a send signed with `nonce` made: a read back that finds the very
/// action. Only `send` and `spotTransfer` rows carry their action's nonce.
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

/// One row and what it did: hl-core's, the shapes the socket sends.
pub use hl_core::ledger::{Delta, LedgerUpdate};

#[cfg(test)]
#[path = "tests/ledger.rs"]
mod tests;
