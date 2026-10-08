//! `spotMeta` and `spotMetaAndAssetCtxs`: tokens, pairs, and what each pair
//! is doing now. Fields the venue adds are left unread.

use hl_core::{Address, ApiError, Asset, Dec, Json, Transport, info};
use hl_trade::Grid;
use serde::Deserialize;

fn request(kind: &str) -> String {
    Json::Object(vec![("type".to_owned(), Json::String(kind.to_owned()))]).compact()
}

/// `{"type":"spotMeta"}`.
#[must_use]
pub fn spot_meta_request() -> String {
    request("spotMeta")
}

/// `{"type":"spotMetaAndAssetCtxs"}`.
#[must_use]
pub fn spot_meta_and_asset_ctxs_request() -> String {
    request("spotMetaAndAssetCtxs")
}

pub async fn spot_meta(transport: &impl Transport) -> Result<SpotMeta, ApiError> {
    info(transport, spot_meta_request()).await
}

pub async fn spot_meta_and_asset_ctxs(transport: &impl Transport) -> Result<SpotMetaAndAssetCtxs, ApiError> {
    info(transport, spot_meta_and_asset_ctxs_request()).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpotMeta {
    pub tokens: Vec<SpotToken>,
    pub universe: Vec<SpotPair>,
}

/// One token as `spotMeta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotToken {
    pub name: String,
    /// What a balance's `token` and `borrowLend` name it by.
    pub index: u32,
    /// `0x` and 32 hex digits.
    pub token_id: String,
    /// The decimals a size on a pair with this base keeps.
    pub sz_decimals: u32,
    /// The decimals the venue counts the token in.
    pub wei_decimals: u32,
    #[serde(default)]
    pub is_canonical: bool,
    /// The linked ERC-20 on HyperEVM, if any.
    #[serde(default)]
    pub evm_contract: Option<EvmContract>,
    #[serde(default)]
    pub full_name: Option<String>,
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
pub struct EvmContract {
    pub address: Address,
    /// The EVM token's decimals less `weiDecimals`, from -2 to 18.
    #[serde(rename = "evm_extra_wei_decimals")]
    pub extra_wei_decimals: i32,
}

/// One pair as `spotMeta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotPair {
    /// What the book and fills call it: `PURR/USDC`, or `@107`.
    pub name: String,
    /// Base, then quote, by token index.
    pub tokens: [u32; 2],
    /// The pair's own index: its asset id is `10000 + index`.
    pub index: u32,
    #[serde(default)]
    pub is_canonical: bool,
}

impl SpotPair {
    #[must_use]
    pub const fn base(&self) -> u32 {
        self.tokens[0]
    }

    #[must_use]
    pub const fn quote(&self) -> u32 {
        self.tokens[1]
    }

    #[must_use]
    pub const fn asset(&self) -> Option<Asset> {
        Asset::spot(self.index)
    }
}

/// A pair with both its tokens: what an order on it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpotMarket<'a> {
    pub pair: &'a SpotPair,
    pub base: &'a SpotToken,
    pub quote: &'a SpotToken,
}

impl SpotMarket<'_> {
    #[must_use]
    pub const fn asset(&self) -> Option<Asset> {
        self.pair.asset()
    }

    /// Prices up to `8 - szDecimals` places of the base, sizes in its
    /// `szDecimals`.
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
    #[must_use]
    pub fn token(&self, index: u32) -> Option<&SpotToken> {
        self.tokens.iter().find(|token| token.index == index)
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
}

/// What a pair is doing now. HIP-4 outcome sides ride in the same list,
/// keyed `#<encoding>`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotCtx {
    /// The pair's or outcome side's book name.
    pub coin: String,
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub prev_day_px: Dec,
    pub day_ntl_vlm: Dec,
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    #[serde(default)]
    pub circulating_supply: Option<Dec>,
    #[serde(default)]
    pub total_supply: Option<Dec>,
}

impl SpotCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }
}

/// `spotMetaAndAssetCtxs`: `[spotMeta, [ctx, ...]]`. The contexts are keyed
/// by `coin`, not by place: they outnumber the pairs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(SpotMeta, Vec<SpotCtx>)")]
pub struct SpotMetaAndAssetCtxs {
    pub meta: SpotMeta,
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
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
