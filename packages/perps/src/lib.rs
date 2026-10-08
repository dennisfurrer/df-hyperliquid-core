//! Hyperliquid perps on the main dex: meta, leverage, margin mode, funding.
//!
//! [`Meta`] names each perp's asset id and grid, so an order goes through
//! `hl-trade` already on it; [`update_leverage`] and the margin moves check
//! what the asset allows before anything is signed. HIP-3 dexes are
//! `hl-hip3`'s.
#![deny(clippy::float_arithmetic)]

pub mod margin;
pub mod meta;

pub use hl_trade::{Grid, MarginMode, Side};
pub use margin::{LeverageError, add_margin, remove_margin, update_leverage};
pub use meta::{
    Meta, MetaAndAssetCtxs, PerpAsset, PerpCtx, meta, meta_and_asset_ctxs, meta_and_asset_ctxs_request, meta_request,
};
