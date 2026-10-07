//! `meta` and `metaAndAssetCtxs` for the main dex: the fields an order needs
//! (name, size decimals, leverage, margin) and the ones a market shows. Every
//! other field the venue sends is left unread.

use hl_core::{ApiError, Asset, AssetKind, Dec, Json, Transport, info};
use hl_trade::Grid;
use serde::Deserialize;

fn request(kind: &str) -> String {
    Json::Object(vec![("type".to_owned(), Json::String(kind.to_owned()))]).compact()
}

/// `{"type":"meta"}`.
#[must_use]
pub fn meta_request() -> String {
    request("meta")
}

/// `{"type":"metaAndAssetCtxs"}`.
#[must_use]
pub fn meta_and_asset_ctxs_request() -> String {
    request("metaAndAssetCtxs")
}

pub async fn meta(transport: &impl Transport) -> Result<Meta, ApiError> {
    info(transport, meta_request()).await
}

pub async fn meta_and_asset_ctxs(transport: &impl Transport) -> Result<MetaAndAssetCtxs, ApiError> {
    info(transport, meta_and_asset_ctxs_request()).await
}

/// The main dex's perps, in the order their asset ids count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub universe: Vec<PerpAsset>,
    /// The spot token perps are margined in; 0 is USDC.
    #[serde(default)]
    pub collateral_token: Option<u32>,
}

/// One perp as `meta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpAsset {
    /// `BTC`: what the order book, fills and positions call it.
    pub name: String,
    pub sz_decimals: u32,
    pub max_leverage: u32,
    #[serde(default)]
    pub margin_table_id: Option<u32>,
    #[serde(default)]
    pub only_isolated: bool,
    /// Delisted perps keep their place, so the ids after them hold.
    #[serde(default)]
    pub is_delisted: bool,
    /// `strictIsolated` or `noCross` where the venue says; absent is cross.
    #[serde(default)]
    pub margin_mode: Option<String>,
}

impl PerpAsset {
    #[must_use]
    pub const fn grid(&self) -> Grid {
        Grid::perp(self.sz_decimals)
    }

    /// Whether a position may share the account's balance. `onlyIsolated`,
    /// `strictIsolated` and `noCross` all say no.
    #[must_use]
    pub fn allows_cross(&self) -> bool {
        !self.only_isolated && !matches!(self.margin_mode.as_deref(), Some("strictIsolated" | "noCross"))
    }
}

impl Meta {
    /// Every perp with its asset id, delisted ones included.
    pub fn perps(&self) -> impl Iterator<Item = (Asset, &PerpAsset)> {
        self.universe
            .iter()
            .enumerate()
            .filter_map(|(index, perp)| Some((Asset::perp(u32::try_from(index).ok()?)?, perp)))
    }

    /// A perp by name, as the venue spells it: `BTC`, not `btc`.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<(Asset, &PerpAsset)> {
        self.perps().find(|(_, perp)| perp.name == name)
    }

    /// A perp by its asset id.
    #[must_use]
    pub fn get(&self, asset: Asset) -> Option<&PerpAsset> {
        match asset.kind() {
            AssetKind::Perp => self.universe.get(usize::try_from(asset.id()).ok()?),
            _ => None,
        }
    }
}

/// What a perp is doing now. The venue leaves `midPx`, `premium` and
/// `impactPxs` out (as null) on a market with no book.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpCtx {
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub oracle_px: Dec,
    pub prev_day_px: Dec,
    /// Hourly, as a fraction of the position's value.
    pub funding: Dec,
    /// In the base asset.
    pub open_interest: Dec,
    /// The quote traded over the last day.
    pub day_ntl_vlm: Dec,
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
    #[serde(default)]
    pub premium: Option<Dec>,
    #[serde(default)]
    pub impact_pxs: Option<Vec<Dec>>,
}

impl PerpCtx {
    /// The price a market order is bounded from: the mid, else the mark.
    #[must_use]
    pub fn reference(&self) -> Dec {
        self.mid_px.unwrap_or(self.mark_px)
    }
}

/// `metaAndAssetCtxs`: `[meta, [ctx, ...]]`, one context per perp, in order.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(Meta, Vec<PerpCtx>)")]
pub struct MetaAndAssetCtxs {
    pub meta: Meta,
    pub ctxs: Vec<PerpCtx>,
}

impl From<(Meta, Vec<PerpCtx>)> for MetaAndAssetCtxs {
    fn from((meta, ctxs): (Meta, Vec<PerpCtx>)) -> Self {
        MetaAndAssetCtxs { meta, ctxs }
    }
}

impl MetaAndAssetCtxs {
    #[must_use]
    pub fn ctx(&self, asset: Asset) -> Option<&PerpCtx> {
        self.meta.get(asset)?;
        self.ctxs.get(usize::try_from(asset.id()).ok()?)
    }

    /// A perp by name with its context.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<(Asset, &PerpAsset, &PerpCtx)> {
        let (asset, perp) = self.meta.find(name)?;
        Some((asset, perp, self.ctx(asset)?))
    }
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
