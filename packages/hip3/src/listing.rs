//! Every perp dex at once: `perpDexs` names them, `allPerpMetas` gives each
//! one's meta, the main dex first in both. A dex's place in the two is the
//! `dex` in its perps' asset ids, so [`Dexes`] keeps them only while they
//! line up: as many metas as dexes, and every coin of a HIP-3 dex's meta
//! named `dex:` after the dex at its place (as hip4-tui's reader checks). An
//! id counted against the wrong dex would sign an order for another market.

use hl_core::{ApiError, Asset, Transport, client::INFO, info};

use crate::{
    dex::{DexAsset, DexMeta, PerpDex, PerpDexs, perp_dexs, request},
    ids::{asset_parts, split_coin},
};

/// `{"type":"allPerpMetas"}`.
#[must_use]
pub fn all_perp_metas_request() -> String {
    request("allPerpMetas", None)
}

/// Every dex's meta in `perpDexs` order, the main dex's first.
pub async fn all_perp_metas(transport: &impl Transport) -> Result<Vec<DexMeta>, ApiError> {
    info(transport, all_perp_metas_request()).await
}

/// `perpDexs`, then `allPerpMetas`, lined up. Read one after the other, so a
/// dex listed between the two reads shows as out of line rather than wrong.
pub async fn dexes(transport: &impl Transport) -> Result<Dexes, ApiError> {
    let list = perp_dexs(transport).await?;
    let metas = all_perp_metas(transport).await?;
    Dexes::new(list, metas).map_err(|error| ApiError::Unreadable { path: INFO, detail: error.to_string() })
}

/// Why `perpDexs` and `allPerpMetas` do not line up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Misaligned {
    #[error("{dexes} dexes and {metas} metas, which should line up one for one")]
    Count { dexes: usize, metas: usize },
    #[error("{coin} is listed under dex {dex}")]
    Coin { coin: String, dex: String },
}

/// Every perp dex with its meta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dexes {
    pub list: PerpDexs,
    /// One a dex, in `perpDexs` order: the main dex's first.
    pub metas: Vec<DexMeta>,
}

impl Dexes {
    pub fn new(list: PerpDexs, metas: Vec<DexMeta>) -> Result<Dexes, Misaligned> {
        if list.0.len() != metas.len() {
            return Err(Misaligned::Count { dexes: list.0.len(), metas: metas.len() });
        }
        for (dex, meta) in list.0.iter().zip(&metas) {
            let Some(dex) = dex else { continue };
            if let Some(stray) =
                meta.universe.iter().find(|perp| split_coin(&perp.name).map(|(of, _)| of) != Some(&dex.name))
            {
                return Err(Misaligned::Coin { coin: stray.name.clone(), dex: dex.name.clone() });
            }
        }
        Ok(Dexes { list, metas })
    }

    /// Each HIP-3 dex with its place and its meta.
    pub fn dexes(&self) -> impl Iterator<Item = (u32, &PerpDex, &DexMeta)> {
        self.list.dexes().filter_map(|(place, dex)| Some((place, dex, self.metas.get(usize::try_from(place).ok()?)?)))
    }

    /// A HIP-3 dex by its name.
    #[must_use]
    pub fn dex(&self, name: &str) -> Option<(u32, &PerpDex, &DexMeta)> {
        self.dexes().find(|(_, dex, _)| dex.name == name)
    }

    /// A HIP-3 perp by its coin, `xyz:TSLA`, with its asset id.
    #[must_use]
    pub fn find(&self, coin: &str) -> Option<(Asset, &DexAsset)> {
        let (name, _) = split_coin(coin)?;
        let (place, _, meta) = self.dex(name)?;
        meta.find(place, coin)
    }

    /// A HIP-3 perp by its asset id, with its dex.
    #[must_use]
    pub fn get(&self, asset: Asset) -> Option<(&PerpDex, &DexAsset)> {
        let (place, _) = asset_parts(asset)?;
        let (_, dex, meta) = self.dexes().find(|(at, _, _)| *at == place)?;
        Some((dex, meta.get(place, asset)?))
    }
}
