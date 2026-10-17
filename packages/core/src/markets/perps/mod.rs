//! Perps on the main dex: meta and contexts, margin tables, order rules and funding.
#![deny(clippy::float_arithmetic)]

pub mod dex;
pub mod funding;
pub mod meta;
pub mod rules;
pub mod table;

pub use crate::trade::{MarginMode, Side};
pub use crate::utils::Grid;
pub use dex::Dex;
pub use funding::{
    FUNDING_INTERVAL_MS, FUNDING_PAGE, FUNDING_WINDOW_MS, FundingDelta, FundingRate, HL_PERP, PredictedFunding,
    PredictedFundings, UserFunding, VenueFunding, funding_history, funding_history_range, funding_history_request,
    funding_windows, merge_rates, next_funding, next_page, predicted_fundings, predicted_fundings_request,
    user_funding, user_funding_range, user_funding_request,
};
pub use meta::{
    Meta, MetaAndAssetCtxs, PerpAsset, PerpCtx, meta, meta_and_asset_ctxs, meta_and_asset_ctxs_request, meta_request,
    perps_at_open_interest_cap, perps_at_open_interest_cap_request,
};
pub use rules::{Rules, underlying};
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
