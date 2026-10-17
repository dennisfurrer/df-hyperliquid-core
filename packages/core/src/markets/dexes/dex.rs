//! `perpDexs`, and `meta` and `metaAndAssetCtxs` for one HIP-3 dex. An unknown dex answers `null`.

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::dexes::ids::asset_parts;
use crate::trade::dexes::MarginTable;
use crate::utils::Grid;
use crate::utils::{Address, Asset, Dec, Json};
use serde::Deserialize;

pub(crate) fn request(kind: &str, dex: Option<&str>) -> String {
    match dex {
        Some(dex) => info_request(kind, &[("dex", Json::String(dex.to_owned()))]),
        None => info_request(kind, &[]),
    }
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

/// `perpDexs`: `null` for the main dex at 0, then each HIP-3 dex. A dex's place here is the `dex`
/// in its asset ids.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct PerpDexs(pub Vec<Option<PerpDex>>);

impl PerpDexs {
    /// Each HIP-3 dex with its place.
    pub fn dexes(&self) -> impl Iterator<Item = (u32, &PerpDex)> {
        self.0.iter().enumerate().filter_map(|(at, dex)| Some((u32::try_from(at).ok()?, dex.as_ref()?)))
    }

    /// A dex's place by its name: `xyz` is 1.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(at, _)| at)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PerpDex> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(_, dex)| dex)
    }

    /// A dex's name by its place; `None` for the main dex at 0.
    #[must_use]
    pub fn name_of(&self, place: u32) -> Option<&str> {
        self.0.get(usize::try_from(place).ok()?)?.as_ref().map(|dex| dex.name.as_str())
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
    /// What each listed coin's funding is scaled by, 0 to 10; unlisted is 1.
    #[serde(default)]
    pub asset_to_funding_multiplier: Vec<(String, Dec)>,
    /// An interest rate of its own in a coin's funding, -0.01 to 0.01.
    #[serde(default)]
    pub asset_to_funding_interest_rate: Vec<(String, Dec)>,
    /// The most a coin's hourly funding may be either way, 0 to 0.01 (`setFundingClamps`).
    #[serde(default)]
    pub asset_to_funding_clamp: Vec<(String, Dec)>,
}

impl PerpDex {
    #[must_use]
    pub fn streaming_oi_cap(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_streaming_oi_cap, coin)
    }

    /// A coin's funding multiplier where the deployer set one.
    #[must_use]
    pub fn funding_multiplier(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_multiplier, coin)
    }

    #[must_use]
    pub fn funding_interest_rate(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_interest_rate, coin)
    }

    /// A coin's funding clamp where the deployer set one.
    #[must_use]
    pub fn funding_clamp(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_clamp, coin)
    }

    /// Who besides the deployer may send `action` (`registerAsset`, `setOracle`, ...) for this dex.
    #[must_use]
    pub fn sub_deployers_for(&self, action: &str) -> &[Address] {
        self.sub_deployers.iter().find(|(listed, _)| listed == action).map_or(&[], |(_, users)| users.as_slice())
    }
}

fn lookup(pairs: &[(String, Dec)], coin: &str) -> Option<Dec> {
    pairs.iter().find(|(listed, _)| listed == coin).map(|(_, value)| *value)
}

/// One dex's perps, in the order their asset ids count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexMeta {
    pub universe: Vec<DexAsset>,
    /// The tiered margin tables by id; a flat one is never listed ([`DexMeta::margin_table`]).
    #[serde(default)]
    pub margin_tables: Vec<(u32, MarginTable)>,
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

    /// A perp by its asset id, for the dex at `dex` in `perpDexs`.
    #[must_use]
    pub fn get(&self, dex: u32, asset: Asset) -> Option<&DexAsset> {
        match asset_parts(asset)? {
            (place, index) if place == dex => self.universe.get(usize::try_from(index).ok()?),
            _ => None,
        }
    }
}

/// What a HIP-3 perp is doing now, the same shape as a main-dex perp's.
pub type DexCtx = crate::markets::context::PerpCtx;

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
