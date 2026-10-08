//! One type a piece, whichever way it arrived. The socket and the `/info` reads say the same
//! things in the types of different crates; each piece is kept in the one every source reads
//! into without losing what it said:
//!
//! - fundings and ledger rows as hl-ws reads them;
//! - a dex's clearinghouse as hl-perps reads it, and from it hl-account's, which usable
//!   balances and worth are read from;
//! - spot balances and the mode as hl-account reads them;

use hl_account::{
    PerpState, SpotBalance, SpotState,
    state::{AssetPosition as HeldPosition, Leverage as HeldLeverage},
};
use hl_perps::{AssetPosition, ClearinghouseState, CumFunding, Leverage, MarginSummary, Position, UserFunding};
use hl_ws::{Funding, Margin};

/// The socket's clearinghouse as hl-perps reads one. An isolated position always carries its
/// `rawUsd`; one that did not would read as none put up.
pub(crate) fn clearinghouse(sent: &hl_ws::Clearinghouse) -> ClearinghouseState {
    let summary = |one: &hl_ws::MarginSummary| MarginSummary {
        account_value: one.account_value,
        total_ntl_pos: one.total_ntl_pos,
        total_raw_usd: one.total_raw_usd,
        total_margin_used: one.total_margin_used,
    };
    let position = |one: &hl_ws::Position| Position {
        coin: one.coin.clone(),
        szi: one.szi,
        leverage: match one.leverage.margin {
            Margin::Cross => Leverage::Cross { value: one.leverage.value },
            Margin::Isolated => {
                Leverage::Isolated { value: one.leverage.value, raw_usd: one.leverage.raw_usd.unwrap_or_default() }
            }
        },
        entry_px: one.entry_px,
        position_value: one.position_value,
        unrealized_pnl: one.unrealized_pnl,
        return_on_equity: one.return_on_equity,
        liquidation_px: one.liquidation_px,
        margin_used: one.margin_used,
        max_leverage: one.max_leverage,
        cum_funding: Some(CumFunding {
            all_time: one.cum_funding.all_time,
            since_open: one.cum_funding.since_open,
            since_change: one.cum_funding.since_change,
        }),
    };
    ClearinghouseState {
        margin_summary: summary(&sent.margin_summary),
        cross_margin_summary: summary(&sent.cross_margin_summary),
        cross_maintenance_margin_used: sent.cross_maintenance_margin_used,
        withdrawable: sent.withdrawable,
        asset_positions: sent
            .asset_positions
            .iter()
            .map(|one| AssetPosition { kind: one.kind.clone(), position: position(&one.position) })
            .collect(),
        time: sent.time,
    }
}

/// A clearinghouse as hl-account reads one, which keeps fewer of its fields.
pub(crate) fn perp_state(state: &ClearinghouseState) -> PerpState {
    let summary = |one: &MarginSummary| hl_account::MarginSummary {
        account_value: one.account_value,
        total_ntl_pos: one.total_ntl_pos,
        total_raw_usd: one.total_raw_usd,
        total_margin_used: one.total_margin_used,
    };
    let position = |one: &Position| hl_account::Position {
        coin: one.coin.clone(),
        szi: one.szi,
        entry_px: one.entry_px,
        position_value: one.position_value,
        unrealized_pnl: one.unrealized_pnl,
        margin_used: one.margin_used,
        liquidation_px: one.liquidation_px,
        leverage: HeldLeverage {
            kind: match one.leverage {
                Leverage::Cross { .. } => "cross".to_owned(),
                Leverage::Isolated { .. } => "isolated".to_owned(),
            },
            value: one.leverage.value(),
            raw_usd: one.leverage.raw_usd(),
        },
        max_leverage: Some(one.max_leverage),
    };
    PerpState {
        margin_summary: summary(&state.margin_summary),
        cross_margin_summary: Some(summary(&state.cross_margin_summary)),
        cross_maintenance_margin_used: state.cross_maintenance_margin_used,
        withdrawable: state.withdrawable,
        asset_positions: state
            .asset_positions
            .iter()
            .map(|one| HeldPosition { position: position(&one.position) })
            .collect(),
        time: Some(state.time),
    }
}

/// The socket's spot balances as hl-account reads them. The socket does not say whether the
/// account is on portfolio margin, nor its ratio: those are kept from what `held` said.
pub(crate) fn spot_state(sent: &hl_ws::SpotState, held: Option<&SpotState>) -> SpotState {
    SpotState {
        balances: sent
            .balances
            .iter()
            .map(|one| SpotBalance {
                coin: one.coin.clone(),
                token: one.token,
                total: one.total,
                hold: one.hold,
                entry_ntl: one.entry_ntl.unwrap_or_default(),
                spot_hold: one.spot_hold,
                borrowed: one.borrowed,
                supplied: one.supplied,
                ltv: one.ltv,
            })
            .collect(),
        portfolio_margin_enabled: held.is_some_and(|held| held.portfolio_margin_enabled),
        portfolio_margin_ratio: held.and_then(|held| held.portfolio_margin_ratio),
        token_to_available_after_maintenance: sent.available_after_maintenance.clone(),
    }
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
