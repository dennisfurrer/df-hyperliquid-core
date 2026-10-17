//! HIP-3 builder-deployed perp dexes: the dex list, each dex's meta and contexts, limits and status.
#![deny(clippy::float_arithmetic)]

pub mod dex;
pub mod ids;
pub mod limits;
pub mod listing;

pub use crate::trade::MarginMode;
pub use crate::utils::Grid;
pub use dex::{
    DexAsset, DexCtx, DexMeta, DexMetaAndAssetCtxs, PerpDex, PerpDexs, dex_meta, dex_meta_and_asset_ctxs,
    dex_meta_and_asset_ctxs_request, dex_meta_request, perp_dexs, perp_dexs_request,
};
pub use ids::{Dex, asset_parts, coin, split_coin};
pub use limits::{
    DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
};
pub use listing::{Dexes, Misaligned, all_perp_metas, all_perp_metas_request, dexes};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;

#[cfg(test)]
#[path = "tests/dexes.rs"]
mod dexes_tests;
