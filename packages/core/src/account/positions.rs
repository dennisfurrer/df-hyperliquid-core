//! `clearinghouseState`: one dex's margin and every position on it, each figure read exactly. A
//! closed position can stay listed at size zero; [`ClearinghouseState::positions`] leaves it out.

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::Dex;
use crate::utils::{Address, Json};

/// `{"type":"clearinghouseState","user":..}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn clearinghouse_state_request(user: Address, dex: Dex<'_>) -> String {
    let fields: Vec<(&str, Json)> = [("user", Json::String(user.to_hex()))].into_iter().chain(dex.field()).collect();
    info_request("clearinghouseState", &fields)
}

pub async fn clearinghouse_state(
    transport: &impl Transport,
    user: Address,
    dex: Dex<'_>,
) -> Result<ClearinghouseState, ApiError> {
    info(transport, clearinghouse_state_request(user, dex)).await
}

/// One dex's clearinghouse for one account, the same shape the socket sends and `subAccounts`
/// carries.
pub type ClearinghouseState = crate::account::clearinghouse::Clearinghouse;

pub use crate::account::clearinghouse::{AssetPosition, CumFunding, Leverage, MarginSummary, Position};
