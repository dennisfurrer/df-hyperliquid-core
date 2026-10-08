//! Hyperliquid spot: token and pair meta, token details, deploy state, spot
//! orders.
//!
//! A token is named three ways and each place wants its own: its index (a
//! balance's `token`, `borrowLend`), `NAME:0x<tokenId>` (a transfer), and its
//! name. A pair is named by its own index, which is not either token's, and
//! trades on its base token's size decimals. Indexes are the venue's and
//! stay put; they are not places in the list, and they differ between the
//! networks (HYPE is 150 on mainnet and 1105 on testnet), so every lookup
//! reads the meta of the network it is on. Balances are `hl-account`'s
//! `spotClearinghouseState`; [`SpotMetaAndAssetCtxs::usdc_price`] values them.
#![deny(clippy::float_arithmetic)]

pub mod deploy;
pub mod details;
pub mod meta;

pub use deploy::{
    DeployingToken, GasAuction, SpotDeployState, TokenSpec, spot_deploy_state, spot_deploy_state_request,
    spot_pair_deploy_auction, spot_pair_deploy_auction_request,
};
pub use details::{Genesis, TokenDetails, token_details, token_details_request};
pub use hl_trade::Grid;
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
