//! Where the socket and the `/info` reads say a piece differently: the socket's spot balances
//! leave out portfolio margin and its ratio, and `userFunding` rows wrap each payment in a
//! `delta` where the socket's are flat.

use crate::account::SpotState;
use crate::live::Funding;
use crate::markets::perps::UserFunding;

/// The socket's spot balances, with portfolio margin and its ratio kept from `held` where the
/// socket does not say them.
pub(crate) fn spot_state(sent: &crate::live::SpotState, held: Option<&SpotState>) -> SpotState {
    let mut spot = sent.state.clone();
    spot.portfolio_margin_enabled =
        spot.portfolio_margin_enabled.or_else(|| held.and_then(|held| held.portfolio_margin_enabled));
    spot.portfolio_margin_ratio =
        spot.portfolio_margin_ratio.or_else(|| held.and_then(|held| held.portfolio_margin_ratio));
    spot
}

/// A read funding row as the socket sends one: its hash is all zeros on every row.
pub(crate) fn funding(read: &UserFunding) -> Funding {
    Funding {
        time: read.time,
        coin: read.delta.coin.clone(),
        usdc: read.delta.usdc,
        szi: read.delta.szi,
        funding_rate: read.delta.funding_rate,
        n_samples: read.delta.n_samples.map(u64::from),
    }
}
