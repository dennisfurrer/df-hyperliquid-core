//! Spot: tokens, pairs and their contexts, and token details.

pub mod details;
pub mod meta;

pub use details::{Genesis, TokenDetails, token_details, token_details_request};
pub use meta::{
    EvmContract, SpotCtx, SpotMarket, SpotMeta, SpotMetaAndAssetCtxs, SpotPair, SpotToken, spot_meta,
    spot_meta_and_asset_ctxs, spot_meta_and_asset_ctxs_request, spot_meta_request,
};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;

#[cfg(test)]
#[path = "tests/reads.rs"]
mod reads;
