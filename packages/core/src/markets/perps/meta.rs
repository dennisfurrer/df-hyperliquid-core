//! `meta`, `metaAndAssetCtxs` and `perpsAtOpenInterestCap` for any perp dex: the main dex, or a
//! HIP-3 dex by name. A perp's asset id counts from its dex's place in `perpDexs`, the main dex at
//! 0. Fields an order or a market view does not need are left unread.

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::dex::Dex;
use crate::markets::perps::table::MarginTable;
use crate::utils::{Asset, AssetKind, Dec, Grid};
use serde::Deserialize;

/// Units of the underlying a `k` perp counts in.
const THOUSAND: Dec = Dec::from_int(1000);

/// `{"type":"meta"}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn meta_request(dex: Dex<'_>) -> String {
    info_request("meta", dex.field().as_slice())
}

/// `{"type":"metaAndAssetCtxs"}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn meta_and_asset_ctxs_request(dex: Dex<'_>) -> String {
    info_request("metaAndAssetCtxs", dex.field().as_slice())
}

/// `None` for a dex the venue does not know, which it answers `null`.
pub async fn meta(transport: &impl Transport, dex: Dex<'_>) -> Result<Option<Meta>, ApiError> {
    info(transport, meta_request(dex)).await
}

/// `None` for a dex the venue does not know, which it answers `null`.
pub async fn meta_and_asset_ctxs(
    transport: &impl Transport,
    dex: Dex<'_>,
) -> Result<Option<MetaAndAssetCtxs>, ApiError> {
    info(transport, meta_and_asset_ctxs_request(dex)).await
}

/// `{"type":"perpsAtOpenInterestCap"}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn perps_at_open_interest_cap_request(dex: Dex<'_>) -> String {
    info_request("perpsAtOpenInterestCap", dex.field().as_slice())
}

/// The perps whose open interest is at its cap, by coin: an order there may only reduce a
/// position.
pub async fn perps_at_open_interest_cap(transport: &impl Transport, dex: Dex<'_>) -> Result<Vec<String>, ApiError> {
    info(transport, perps_at_open_interest_cap_request(dex)).await
}

/// One dex's perps, in the order their asset ids count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Meta {
    /// Every perp, in asset id order, delisted ones included.
    pub universe: Vec<PerpAsset>,
    /// The tiered margin tables by id; a flat one is never listed ([`Meta::margin_table`]).
    #[serde(default)]
    pub margin_tables: Vec<(u32, MarginTable)>,
    /// The spot token the dex margins in; 0 is USDC.
    #[serde(default)]
    pub collateral_token: Option<u32>,
}

/// One perp as `meta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PerpAsset {
    /// What the order book, fills and positions call it: `BTC`, or `xyz:TSLA` on a HIP-3 dex.
    pub name: String,
    /// Size decimals (`szDecimals`): how many decimal places an order size may have.
    pub sz_decimals: u32,
    /// The highest leverage allowed (`maxLeverage`).
    pub max_leverage: u32,
    /// The margin table the perp uses (`marginTableId`); `None` when the venue leaves it out.
    #[serde(default)]
    pub margin_table_id: Option<u32>,
    /// Whether the perp is isolated margin only (`onlyIsolated`).
    #[serde(default)]
    pub only_isolated: bool,
    /// Delisted perps keep their place, so the ids after them hold.
    #[serde(default)]
    pub is_delisted: bool,
    /// `strictIsolated` or `noCross` where the venue or the deployer says; absent is cross.
    #[serde(default)]
    pub margin_mode: Option<String>,
    /// `enabled` where a HIP-3 deployer turned growth mode on.
    #[serde(default)]
    pub growth_mode: Option<String>,
    /// A HIP-3 deployer's share of fees, set per asset.
    #[serde(default)]
    pub deployer_fee_scale: Option<Dec>,
}

impl PerpAsset {
    /// The price and size grid orders on this perp must sit on.
    #[must_use]
    pub const fn grid(&self) -> Grid {
        Grid::perp(self.sz_decimals)
    }

    /// Whether cross margin is allowed. `onlyIsolated`, `strictIsolated` and `noCross` all refuse it.
    #[must_use]
    pub fn allows_cross(&self) -> bool {
        !self.only_isolated && !matches!(self.margin_mode.as_deref(), Some("strictIsolated" | "noCross"))
    }
}

/// What a coin tracks and how many units of it one contract is: `kPEPE` is a thousand PEPE. A
/// HIP-3 coin's `dex:` is left off: `xyz:TSLA` tracks TSLA.
#[must_use]
pub fn underlying(coin: &str) -> (&str, Dec) {
    let bare = coin.split_once(':').map_or(coin, |(_, symbol)| symbol);
    match bare.strip_prefix('k') {
        Some(rest) if rest.starts_with(|first: char| first.is_ascii_uppercase()) => (rest, THOUSAND),
        _ => (bare, Dec::ONE),
    }
}

/// A perp asset id's dex place and index: `(0, index)` on the main dex.
const fn place_of(asset: Asset) -> Option<(u32, u32)> {
    match asset.kind() {
        AssetKind::Perp => Some((0, asset.id())),
        _ => asset.hip3_parts(),
    }
}

impl Meta {
    /// Every perp with its asset id, delisted ones included, for the dex at `place` in `perpDexs`:
    /// the main dex at 0.
    pub fn perps(&self, place: u32) -> impl Iterator<Item = (Asset, &PerpAsset)> {
        self.universe
            .iter()
            .enumerate()
            .filter_map(move |(index, perp)| Some((Asset::perp_on(place, u32::try_from(index).ok()?)?, perp)))
    }

    /// A perp by name as the venue spells it (`BTC`, not `btc`; `xyz:TSLA`), on the dex at `place`.
    #[must_use]
    pub fn find(&self, place: u32, name: &str) -> Option<(Asset, &PerpAsset)> {
        self.perps(place).find(|(_, perp)| perp.name == name)
    }

    /// A perp by its asset id, on the dex at `place`.
    #[must_use]
    pub fn get(&self, place: u32, asset: Asset) -> Option<&PerpAsset> {
        match place_of(asset)? {
            (at, index) if at == place => self.universe.get(usize::try_from(index).ok()?),
            _ => None,
        }
    }
}

/// What a perp is doing now, the same shape the socket sends.
pub use crate::markets::context::PerpCtx;

/// `metaAndAssetCtxs`: `[meta, [ctx, ...]]`, one context per perp, in order.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(Meta, Vec<PerpCtx>)")]
#[non_exhaustive]
pub struct MetaAndAssetCtxs {
    /// The dex's perps.
    pub meta: Meta,
    /// One context per perp, in the order of `meta.universe`.
    pub ctxs: Vec<PerpCtx>,
}

impl From<(Meta, Vec<PerpCtx>)> for MetaAndAssetCtxs {
    fn from((meta, ctxs): (Meta, Vec<PerpCtx>)) -> Self {
        MetaAndAssetCtxs { meta, ctxs }
    }
}

impl MetaAndAssetCtxs {
    /// A perp's context by its asset id, on the dex at `place`.
    #[must_use]
    pub fn ctx(&self, place: u32, asset: Asset) -> Option<&PerpCtx> {
        self.meta.get(place, asset)?;
        let (_, index) = place_of(asset)?;
        self.ctxs.get(usize::try_from(index).ok()?)
    }

    /// A perp by name with its context, on the dex at `place`.
    #[must_use]
    pub fn find(&self, place: u32, name: &str) -> Option<(Asset, &PerpAsset, &PerpCtx)> {
        let (asset, perp) = self.meta.find(place, name)?;
        Some((asset, perp, self.ctx(place, asset)?))
    }
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
