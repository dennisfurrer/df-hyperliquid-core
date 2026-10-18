//! Perps on every dex, the main one and HIP-3 ones alike: meta and contexts, margin tables and
//! funding.

pub mod funding;
pub mod meta;
pub mod table;

pub use funding::{
    FUNDING_INTERVAL_MS, FUNDING_PAGE, FUNDING_WINDOW_MS, FundingDelta, FundingRate, HL_PERP, PredictedFunding,
    PredictedFundings, UserFunding, VenueFunding, funding_history, funding_history_range, funding_history_request,
    funding_windows, merge_rates, next_funding, next_page, predicted_fundings, predicted_fundings_request,
    user_funding, user_funding_range, user_funding_request,
};
pub use meta::{
    Meta, MetaAndAssetCtxs, PerpAsset, PerpCtx, meta, meta_and_asset_ctxs, meta_and_asset_ctxs_request, meta_request,
    perps_at_open_interest_cap, perps_at_open_interest_cap_request, underlying,
};
pub use table::{FLAT_TABLES_BELOW, MarginTable, MarginTier};

#[cfg(test)]
#[path = "tests/funding.rs"]
mod funding_tests;
