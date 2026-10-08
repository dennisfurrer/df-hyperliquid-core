//! `clearinghouseState`: one dex's margin and every position on it. Each
//! figure is the venue's decimal string, read exactly. A position the account
//! has closed can stay listed at size zero; [`ClearinghouseState::positions`]
//! leaves it out.

use hl_core::{Address, ApiError, Transport, info};

use crate::dex::{Dex, request, text};

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

/// One dex's clearinghouse for one account, its margin and its positions: hl-core's, the shapes
/// the socket sends and `subAccounts` carries.
pub type ClearinghouseState = hl_core::state::Clearinghouse;

pub use hl_core::state::{AssetPosition, CumFunding, Leverage, MarginSummary, Position};
