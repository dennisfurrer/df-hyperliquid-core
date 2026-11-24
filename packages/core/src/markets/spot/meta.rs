//! `spotMeta` and `spotMetaAndAssetCtxs`: tokens, pairs, and what each pair is doing now. Fields
//! the venue adds are left unread.

use crate::client::{ApiError, Transport, USDC_INDEX, info, info_request};
use crate::utils::Grid;
use crate::utils::{Address, Asset, AssetKind, Dec, SPOT_ASSET_BASE};
use serde::Deserialize;

/// `{"type":"spotMeta"}`.
#[must_use]
pub fn spot_meta_request() -> String {
    info_request("spotMeta", &[])
}

/// `{"type":"spotMetaAndAssetCtxs"}`.
#[must_use]
pub fn spot_meta_and_asset_ctxs_request() -> String {
    info_request("spotMetaAndAssetCtxs", &[])
}

/// Every spot token and pair.
pub async fn spot_meta(transport: &impl Transport) -> Result<SpotMeta, ApiError> {
    info(transport, spot_meta_request()).await
}

/// Every spot token and pair, with what each pair is doing now.
pub async fn spot_meta_and_asset_ctxs(transport: &impl Transport) -> Result<SpotMetaAndAssetCtxs, ApiError> {
    info(transport, spot_meta_and_asset_ctxs_request()).await
}

/// Tokens and pairs. Indexes are the venue's, not places in the list, and differ between networks
/// (HYPE is 150 on mainnet and 1105 on testnet).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct SpotMeta {
    /// Every token.
    pub tokens: Vec<SpotToken>,
    /// Every pair.
    pub universe: Vec<SpotPair>,
}

/// One token as `spotMeta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SpotToken {
    /// The token's name.
    pub name: String,
    /// What a balance's `token` and `borrowLend` name it by.
    pub index: u32,
    /// `0x` and 32 hex digits.
    pub token_id: String,
    /// The decimals a size on a pair with this base keeps.
    pub sz_decimals: u32,
    /// The decimals the venue counts the token in.
    pub wei_decimals: u32,
    /// Whether the token is canonical (`isCanonical`): one of the venue's original tokens, such as USDC
    /// and PURR.
    #[serde(default)]
    pub is_canonical: bool,
    /// The linked ERC-20 on HyperEVM, if any.
    #[serde(default)]
    pub evm_contract: Option<EvmContract>,
    /// The token's full name (`fullName`); `None` where none was set.
    #[serde(default)]
    pub full_name: Option<String>,
    /// The deployer's share of the fees its pairs make, 0 to 1.
    #[serde(default)]
    pub deployer_trading_fee_share: Option<Dec>,
    /// A short label its deployer set, on the few tokens that carry one (RAT's is `rrc`).
    #[serde(default)]
    pub deployer_label: Option<String>,
}

impl SpotToken {
    /// `NAME:0x<tokenId>`, the one way a transfer names a token.
    #[must_use]
    pub fn wire(&self) -> String {
        format!("{}:{}", self.name, self.token_id)
    }
}

/// A token's ERC-20 on HyperEVM. The venue spells this key in snake case.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct EvmContract {
    /// The ERC-20's address on HyperEVM.
    pub address: Address,
    /// The EVM token's decimals less `weiDecimals`, from -2 to 18.
    #[serde(rename = "evm_extra_wei_decimals")]
    pub extra_wei_decimals: i32,
}

/// One pair as `spotMeta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SpotPair {
    /// What the book and fills call it: `PURR/USDC`, or `@107`.
    pub name: String,
    /// Base, then quote, by token index.
    pub tokens: [u32; 2],
    /// The pair's own index: its asset id is `10000 + index`.
    pub index: u32,
    /// Whether the pair is canonical (`isCanonical`): named `BASE/QUOTE` rather than `@index`.
    #[serde(default)]
    pub is_canonical: bool,
}

impl SpotPair {
    /// The base token's index.
    #[must_use]
    pub const fn base(&self) -> u32 {
        self.tokens[0]
    }

    /// The quote token's index.
    #[must_use]
    pub const fn quote(&self) -> u32 {
        self.tokens[1]
    }

    /// The pair's asset id, `10000 + index`; `None` past the asset id range.
    #[must_use]
    pub const fn asset(&self) -> Option<Asset> {
        Asset::spot(self.index)
    }
}

/// A pair with both its tokens: what an order on it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpotMarket<'a> {
    /// The pair.
    pub pair: &'a SpotPair,
    /// The base token.
    pub base: &'a SpotToken,
    /// The quote token.
    pub quote: &'a SpotToken,
}

impl SpotMarket<'_> {
    /// The pair's asset id, `10000 + index`; `None` past the asset id range.
    #[must_use]
    pub const fn asset(&self) -> Option<Asset> {
        self.pair.asset()
    }

    /// Prices at most `8 - szDecimals` places, sizes on the base token's `szDecimals`.
    #[must_use]
    pub const fn grid(&self) -> Grid {
        Grid::spot(self.base.sz_decimals)
    }

    /// `HYPE/USDC`, whatever the book calls it.
    #[must_use]
    pub fn display_name(&self) -> String {
        format!("{}/{}", self.base.name, self.quote.name)
    }
}

impl SpotMeta {
    /// By the venue's index for it, which is not its place in the list.
    #[must_use]
    pub fn token(&self, index: u32) -> Option<&SpotToken> {
        self.tokens.iter().find(|token| token.index == index)
    }

    /// USDC, at index 0 on both networks; its token id is each network's own.
    #[must_use]
    pub fn usdc(&self) -> Option<&SpotToken> {
        self.token(USDC_INDEX)
    }

    /// By token id, `0x` and 32 hex digits in either case.
    #[must_use]
    pub fn token_by_id(&self, token_id: &str) -> Option<&SpotToken> {
        self.tokens.iter().find(|token| token.token_id.eq_ignore_ascii_case(token_id))
    }

    /// By name, as the venue spells it: `HYPE`.
    #[must_use]
    pub fn token_named(&self, name: &str) -> Option<&SpotToken> {
        self.tokens.iter().find(|token| token.name == name)
    }

    /// By `NAME:0x<tokenId>`, the id in either case.
    #[must_use]
    pub fn token_by_wire(&self, wire: &str) -> Option<&SpotToken> {
        let (name, id) = wire.split_once(':')?;
        self.tokens.iter().find(|token| token.name == name && token.token_id.eq_ignore_ascii_case(id))
    }

    /// By the pair's own index.
    #[must_use]
    pub fn pair(&self, index: u32) -> Option<&SpotPair> {
        self.universe.iter().find(|pair| pair.index == index)
    }

    /// By the name the book calls it: `PURR/USDC`, `@107`.
    #[must_use]
    pub fn pair_named(&self, name: &str) -> Option<&SpotPair> {
        self.universe.iter().find(|pair| pair.name == name)
    }

    /// By its two tokens' indexes, base first.
    #[must_use]
    pub fn pair_of(&self, base: u32, quote: u32) -> Option<&SpotPair> {
        self.universe.iter().find(|pair| pair.tokens == [base, quote])
    }

    /// A pair with its tokens, by the pair's index.
    #[must_use]
    pub fn market(&self, index: u32) -> Option<SpotMarket<'_>> {
        let pair = self.pair(index)?;
        Some(SpotMarket { pair, base: self.token(pair.base())?, quote: self.token(pair.quote())? })
    }

    /// A pair with its tokens, by the tokens' names: `("HYPE", "USDC")`.
    #[must_use]
    pub fn market_of(&self, base: &str, quote: &str) -> Option<SpotMarket<'_>> {
        let pair = self.pair_of(self.token_named(base)?.index, self.token_named(quote)?.index)?;
        self.market(pair.index)
    }

    /// A pair with its tokens, by the name the book and fills call it: `PURR/USDC`, `@107`.
    #[must_use]
    pub fn market_named(&self, name: &str) -> Option<SpotMarket<'_>> {
        self.market(self.pair_named(name)?.index)
    }

    /// A pair with its tokens, by its asset id (`10000 + index`), as an order status or a fill
    /// names it.
    #[must_use]
    pub fn market_at(&self, asset: Asset) -> Option<SpotMarket<'_>> {
        match asset.kind() {
            AssetKind::Spot => self.market(asset.id().checked_sub(SPOT_ASSET_BASE)?),
            _ => None,
        }
    }
}

/// What a pair is doing now, the same shape the socket sends. HIP-4 outcome sides share the list,
/// keyed `#<encoding>`.
pub use crate::markets::context::SpotCtx;

/// `spotMetaAndAssetCtxs`: `[spotMeta, [ctx, ...]]`. The contexts are keyed by `coin`, not by
/// place: they outnumber the pairs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(SpotMeta, Vec<SpotCtx>)")]
#[non_exhaustive]
pub struct SpotMetaAndAssetCtxs {
    /// Every token and pair.
    pub meta: SpotMeta,
    /// Every context, keyed by `coin`: the pairs' and the outcome sides'.
    pub ctxs: Vec<SpotCtx>,
}

impl From<(SpotMeta, Vec<SpotCtx>)> for SpotMetaAndAssetCtxs {
    fn from((meta, ctxs): (SpotMeta, Vec<SpotCtx>)) -> Self {
        SpotMetaAndAssetCtxs { meta, ctxs }
    }
}

impl SpotMetaAndAssetCtxs {
    /// By book name: `PURR/USDC`, `@107`, `#12100`.
    #[must_use]
    pub fn ctx(&self, coin: &str) -> Option<&SpotCtx> {
        self.ctxs.iter().find(|ctx| ctx.coin == coin)
    }

    /// A pair with its tokens and its context, by the pair's index.
    #[must_use]
    pub fn market_ctx(&self, index: u32) -> Option<(SpotMarket<'_>, &SpotCtx)> {
        let market = self.meta.market(index)?;
        Some((market, self.ctx(&market.pair.name)?))
    }

    /// What one of a token is worth in USDC: the mark of its pair with USDC, 1 for USDC itself.
    /// `None` for a token with no such pair, or none trading.
    #[must_use]
    pub fn usdc_price(&self, token: u32) -> Option<Dec> {
        if token == USDC_INDEX {
            return Some(Dec::ONE);
        }
        let pair = self.meta.pair_of(token, USDC_INDEX)?;
        Some(self.market_ctx(pair.index)?.1.mark_px)
    }
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
