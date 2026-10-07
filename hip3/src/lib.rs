//! HIP-3 builder-deployed perp dexes: dex list, dex asset ids, per-dex meta, deployer actions.
//!
//! A HIP-3 perp's asset id is `100000 + dex * 10000 + index`: `dex` its dex's
//! place in `perpDexs` ([`PerpDexs::index_of`]), `index` its place in that
//! dex's `meta`. Coins are `dex:SYMBOL`. Orders, leverage and margin are
//! `hl-trade`'s, as on the main dex.
//!
//! No deployer (`perpDeploy`) action is here: no reference pins one byte for
//! byte yet, and a deploy's stake and auction are not undone.
#![deny(clippy::float_arithmetic)]

pub mod dex;

pub use dex::{
    DexAsset, DexCtx, DexMeta, DexMetaAndAssetCtxs, PerpDex, PerpDexs, dex_meta, dex_meta_and_asset_ctxs,
    dex_meta_and_asset_ctxs_request, dex_meta_request, perp_dexs, perp_dexs_request,
};
pub use hl_trade::{Grid, MarginMode};
