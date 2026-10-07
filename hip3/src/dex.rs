//! `perpDexs`, and `meta` and `metaAndAssetCtxs` with `dex`. A dex the venue
//! does not know answers `null`.

use hl_core::{Address, ApiError, Asset, Dec, Json, Transport, info};
use hl_trade::Grid;
use serde::Deserialize;

fn request(kind: &str, dex: Option<&str>) -> String {
    let mut fields = vec![("type".to_owned(), Json::String(kind.to_owned()))];
    if let Some(dex) = dex {
        fields.push(("dex".to_owned(), Json::String(dex.to_owned())));
    }
    Json::Object(fields).compact()
}

/// `{"type":"perpDexs"}`.
#[must_use]
pub fn perp_dexs_request() -> String {
    request("perpDexs", None)
}

/// `{"type":"meta","dex":<name>}`.
#[must_use]
pub fn dex_meta_request(dex: &str) -> String {
    request("meta", Some(dex))
}

/// `{"type":"metaAndAssetCtxs","dex":<name>}`.
#[must_use]
pub fn dex_meta_and_asset_ctxs_request(dex: &str) -> String {
    request("metaAndAssetCtxs", Some(dex))
}

pub async fn perp_dexs(transport: &impl Transport) -> Result<PerpDexs, ApiError> {
    info(transport, perp_dexs_request()).await
}

/// `None` for a dex the venue does not know.
pub async fn dex_meta(transport: &impl Transport, dex: &str) -> Result<Option<DexMeta>, ApiError> {
    info(transport, dex_meta_request(dex)).await
}

/// `None` for a dex the venue does not know.
pub async fn dex_meta_and_asset_ctxs(
    transport: &impl Transport,
    dex: &str,
) -> Result<Option<DexMetaAndAssetCtxs>, ApiError> {
    info(transport, dex_meta_and_asset_ctxs_request(dex)).await
}

/// `perpDexs`: `null` for the main dex at 0, then each HIP-3 dex. A dex's
/// place here is the `dex` in its assets' ids.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct PerpDexs(pub Vec<Option<PerpDex>>);

impl PerpDexs {
    /// Each HIP-3 dex with its place.
    pub fn dexes(&self) -> impl Iterator<Item = (u32, &PerpDex)> {
        self.0.iter().enumerate().filter_map(|(at, dex)| Some((u32::try_from(at).ok()?, dex.as_ref()?)))
    }

    /// A dex's place by its name: `xyz` is 1 while it is first.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(at, _)| at)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PerpDex> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(_, dex)| dex)
    }
}

/// One HIP-3 dex as `perpDexs` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDex {
    /// 2 to 4 lower case letters, the `dex:` of its coins.
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub deployer: Option<Address>,
    #[serde(default)]
    pub oracle_updater: Option<Address>,
    #[serde(default)]
    pub fee_recipient: Option<Address>,
    /// Each deploy action the deployer lets others send, and who.
    #[serde(default)]
    pub sub_deployers: Vec<(String, Vec<Address>)>,
    /// The open interest cap each coin streams toward, in the quote.
    #[serde(default)]
    pub asset_to_streaming_oi_cap: Vec<(String, Dec)>,
    #[serde(default)]
    pub asset_to_funding_multiplier: Vec<(String, Dec)>,
}

/// One dex's perps, in the order their asset ids count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexMeta {
    pub universe: Vec<DexAsset>,
    /// The spot token the dex margins in; 0 is USDC.
    #[serde(default)]
    pub collateral_token: Option<u32>,
}

/// One perp of a HIP-3 dex.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexAsset {
    /// `xyz:TSLA`.
    pub name: String,
    pub sz_decimals: u32,
    pub max_leverage: u32,
    #[serde(default)]
    pub margin_table_id: Option<u32>,
    #[serde(default)]
    pub only_isolated: bool,
    #[serde(default)]
    pub is_delisted: bool,
    /// `strictIsolated` or `noCross` where the deployer set one.
    #[serde(default)]
    pub margin_mode: Option<String>,
    /// `enabled` where the deployer turned growth mode on.
    #[serde(default)]
    pub growth_mode: Option<String>,
    /// The deployer's share of fees, set per asset.
    #[serde(default)]
    pub deployer_fee_scale: Option<Dec>,
}

impl DexAsset {
    #[must_use]
    pub const fn grid(&self) -> Grid {
        Grid::perp(self.sz_decimals)
    }

    /// `onlyIsolated`, `strictIsolated` and `noCross` all refuse cross.
    #[must_use]
    pub fn allows_cross(&self) -> bool {
        !self.only_isolated && !matches!(self.margin_mode.as_deref(), Some("strictIsolated" | "noCross"))
    }
}

impl DexMeta {
    /// Every perp with its asset id, for the dex at `dex` in `perpDexs`.
    pub fn perps(&self, dex: u32) -> impl Iterator<Item = (Asset, &DexAsset)> {
        self.universe
            .iter()
            .enumerate()
            .filter_map(move |(index, perp)| Some((Asset::hip3(dex, u32::try_from(index).ok()?)?, perp)))
    }

    /// A perp by its full coin, `xyz:TSLA`.
    #[must_use]
    pub fn find(&self, dex: u32, coin: &str) -> Option<(Asset, &DexAsset)> {
        self.perps(dex).find(|(_, perp)| perp.name == coin)
    }
}

/// What a HIP-3 perp is doing now; the main dex's shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexCtx {
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub oracle_px: Dec,
    pub prev_day_px: Dec,
    /// Hourly, as a fraction of the position's value.
    pub funding: Dec,
    pub open_interest: Dec,
    pub day_ntl_vlm: Dec,
}

impl DexCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }
}

/// `metaAndAssetCtxs` with `dex`: `[meta, [ctx, ...]]`, one context per perp.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(DexMeta, Vec<DexCtx>)")]
pub struct DexMetaAndAssetCtxs {
    pub meta: DexMeta,
    pub ctxs: Vec<DexCtx>,
}

impl From<(DexMeta, Vec<DexCtx>)> for DexMetaAndAssetCtxs {
    fn from((meta, ctxs): (DexMeta, Vec<DexCtx>)) -> Self {
        DexMetaAndAssetCtxs { meta, ctxs }
    }
}

impl DexMetaAndAssetCtxs {
    /// A perp by its full coin with its context.
    #[must_use]
    pub fn find(&self, dex: u32, coin: &str) -> Option<(Asset, &DexAsset, &DexCtx)> {
        let index = self.meta.universe.iter().position(|perp| perp.name == coin)?;
        let asset = Asset::hip3(dex, u32::try_from(index).ok()?)?;
        Some((asset, self.meta.universe.get(index)?, self.ctxs.get(index)?))
    }
}

#[cfg(test)]
#[path = "tests/dex.rs"]
mod tests;
