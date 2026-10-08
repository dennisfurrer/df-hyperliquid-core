//! HIP-3 builder-deployed perp dexes: the dex list and every dex's meta,
//! dex asset ids, per-dex meta and contexts, limits, status, funding
//! settings, margin tables and leverage, and the deploy auction.
//!
//! A HIP-3 perp's asset id is `100000 + dex * 10000 + index`: `dex` its dex's
//! place in `perpDexs` ([`PerpDexs::index_of`]), `index` its place in that
//! dex's `meta`. Coins are `dex:SYMBOL` ([`split_coin`]). [`Dexes`] lines
//! `perpDexs` up with `allPerpMetas` and refuses the two out of line, since
//! an id counted against the wrong dex names another market. Orders and
//! margin moves are `hl-trade`'s, as on the main dex; positions and funding
//! are `hl-perps`'s, which take a dex.
//!
//! No deployer (`perpDeploy`) action is here: no independent reference pins
//! one byte for byte yet (the venue's Python SDK has no vector for it, and
//! its `registerAsset` predates `registerAsset2`), and a deploy's stake and
//! auction are not undone.
#![deny(clippy::float_arithmetic)]

pub mod auction;
pub mod dex;
pub mod ids;
pub mod limits;
pub mod listing;
pub mod margin;

pub use auction::{DeployAuction, perp_deploy_auction, perp_deploy_auction_request};
pub use dex::{
    DexAsset, DexCtx, DexMeta, DexMetaAndAssetCtxs, PerpDex, PerpDexs, dex_meta, dex_meta_and_asset_ctxs,
    dex_meta_and_asset_ctxs_request, dex_meta_request, perp_dexs, perp_dexs_request,
};
pub use hl_trade::{Grid, MarginMode};
pub use ids::{asset_parts, coin, split_coin};
pub use limits::{
    DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
};
pub use listing::{Dexes, Misaligned, all_perp_metas, all_perp_metas_request, dexes};
pub use margin::{FLAT_TABLES_BELOW, LeverageError, MarginTable, MarginTier, update_leverage, update_leverage_for};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;

#[cfg(test)]
#[path = "tests/dexes.rs"]
mod dexes_tests;
