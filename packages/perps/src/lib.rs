//! Hyperliquid perps on the main dex: meta, margin tables, leverage, margin
//! mode, positions, funding.
//!
//! [`Meta`] names each perp's asset id and grid, so an order goes through
//! `hl-trade` already on it; [`update_leverage`] and the margin moves check
//! what the asset allows before anything is signed, and
//! [`update_leverage_for`] the tier a position's notional reaches as well.
//!
//! The account reads (`clearinghouseState`, `activeAssetData`, `userFunding`)
//! and `perpsAtOpenInterestCap` serve a HIP-3 dex too: [`Dex`] names which.
//! A HIP-3 dex's own meta, whose asset ids count from its place in
//! `perpDexs`, is `hl-hip3`'s.
#![deny(clippy::float_arithmetic)]

pub mod active;
pub mod dex;
pub mod funding;
pub mod margin;
pub mod meta;
pub mod rules;
pub mod state;
pub mod table;

pub use active::{ActiveAssetData, active_asset_data, active_asset_data_request};
pub use dex::Dex;
pub use funding::{
    FUNDING_INTERVAL_MS, FUNDING_PAGE, FUNDING_WINDOW_MS, FundingDelta, FundingRate, HL_PERP, PredictedFunding,
    PredictedFundings, UserFunding, VenueFunding, funding_history, funding_history_range, funding_history_request,
    funding_windows, merge_rates, next_funding, next_page, predicted_fundings, predicted_fundings_request,
    user_funding, user_funding_range, user_funding_request,
};
pub use hl_trade::{Grid, MarginMode, Side};
pub use margin::{LeverageError, add_margin, remove_margin, update_leverage, update_leverage_for};
pub use meta::{
    Meta, MetaAndAssetCtxs, PerpAsset, PerpCtx, meta, meta_and_asset_ctxs, meta_and_asset_ctxs_request, meta_request,
    perps_at_open_interest_cap, perps_at_open_interest_cap_request,
};
pub use rules::{Rules, underlying};
pub use state::{
    AssetPosition, ClearinghouseState, CumFunding, Leverage, MarginSummary, Position, clearinghouse_state,
    clearinghouse_state_request,
};
pub use table::{FLAT_TABLES_BELOW, MarginTable, MarginTier};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;

#[cfg(test)]
#[path = "tests/state.rs"]
mod state_tests;

#[cfg(test)]
#[path = "tests/funding.rs"]
mod funding_tests;

#[cfg(test)]
#[path = "tests/rules.rs"]
mod rules_tests;
