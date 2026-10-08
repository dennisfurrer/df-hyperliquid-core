//! One type a piece, whichever way it arrived. The socket and the `/info` reads say the same
//! things in the types of different crates; each piece is kept in the one every source reads
//! into without losing what it said:
//!
//! - fundings and ledger rows as hl-ws reads them;
//! - spot balances and the mode as hl-account reads them;

use hl_account::SpotState;
use hl_perps::UserFunding;
use hl_ws::Funding;

/// The socket's spot balances, which are hl-account's type already. The socket does not say
/// whether the account is on portfolio margin, nor its ratio: where it does not, those are kept
/// from what `held` said.
pub(crate) fn spot_state(sent: &hl_ws::SpotState, held: Option<&SpotState>) -> SpotState {
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
