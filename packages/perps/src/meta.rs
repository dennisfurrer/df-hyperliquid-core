//! `meta` and `metaAndAssetCtxs` for the main dex: the fields an order needs
//! (name, size decimals, leverage, margin, the margin tables) and the ones a
//! market shows; and `perpsAtOpenInterestCap`. Every other field the venue
//! sends is left unread.

use hl_core::{ApiError, Asset, AssetKind, Transport, info, info_request};
use hl_trade::Grid;
use serde::Deserialize;

use crate::{
    dex::{self, Dex},
    table::MarginTable,
};

fn request(kind: &str) -> String {
    info_request(kind, &[])
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

/// `{"type":"perpsAtOpenInterestCap"}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn perps_at_open_interest_cap_request(dex: Dex<'_>) -> String {
    dex::request("perpsAtOpenInterestCap", Vec::new(), dex)
}

/// The perps whose open interest is at its cap, by coin: an order there may
/// only reduce a position.
pub async fn perps_at_open_interest_cap(transport: &impl Transport, dex: Dex<'_>) -> Result<Vec<String>, ApiError> {
    info(transport, perps_at_open_interest_cap_request(dex)).await
}

/// The main dex's perps, in the order their asset ids count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub universe: Vec<PerpAsset>,
    /// The tiered margin tables by id; a flat one is never listed
    /// ([`Meta::margin_table`]).
    #[serde(default)]
    pub margin_tables: Vec<(u32, MarginTable)>,
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

/// What a perp is doing now: hl-core's, the shape the socket sends.
pub use hl_core::context::PerpCtx;

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
