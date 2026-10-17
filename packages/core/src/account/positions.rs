//! `clearinghouseState`: one dex's margin and every position on it, each figure read exactly. A
//! closed position can stay listed at size zero; [`ClearinghouseState::positions`] leaves it out.

use crate::client::{ApiError, Transport, info};
use crate::markets::perps::dex::{Dex, request, text};
use crate::utils::Address;

/// `{"type":"clearinghouseState","user":..}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn clearinghouse_state_request(user: Address, dex: Dex<'_>) -> String {
    request("clearinghouseState", vec![("user", text(&user.to_hex()))], dex)
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
