//! Hyperliquid spot: token and pair meta, balances, spot orders.
//!
//! A token is named three ways and each place wants its own: its index (a
//! balance's `token`, `borrowLend`), `NAME:0x<tokenId>` (a transfer), and its
//! name. A pair is named by its own index, which is not either token's, and
//! trades on its base token's size decimals. Balances are `hl-account`'s
//! `spotClearinghouseState`.
#![deny(clippy::float_arithmetic)]

pub mod meta;

pub use hl_trade::Grid;
pub use meta::{
    EvmContract, SpotCtx, SpotMarket, SpotMeta, SpotMetaAndAssetCtxs, SpotPair, SpotToken, spot_meta,
    spot_meta_and_asset_ctxs, spot_meta_and_asset_ctxs_request, spot_meta_request,
};
