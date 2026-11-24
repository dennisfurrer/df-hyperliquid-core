//! Every perp dex at once: `perpDexs` lined up with `allPerpMetas`, the main dex first in both.
//! [`Dexes`] refuses the two out of line (a count mismatch, or a coin not named after its dex), since
//! an asset id counted against the wrong dex would sign an order for another market.

use crate::client::http::transport::INFO;
use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::dex::split_coin;
use crate::markets::dexes::dex::{PerpDex, PerpDexs, perp_dexs};
use crate::markets::perps::meta::{Meta, PerpAsset};
use crate::utils::Asset;

/// `{"type":"allPerpMetas"}`.
#[must_use]
pub fn all_perp_metas_request() -> String {
    info_request("allPerpMetas", &[])
}

/// Every dex's meta in `perpDexs` order, the main dex's first.
pub async fn all_perp_metas(transport: &impl Transport) -> Result<Vec<Meta>, ApiError> {
    info(transport, all_perp_metas_request()).await
}

/// `perpDexs`, then `allPerpMetas`, lined up. A dex listed between the two reads shows as out of
/// line rather than wrong.
pub async fn dexes(transport: &impl Transport) -> Result<Dexes, ApiError> {
    let list = perp_dexs(transport).await?;
    let metas = all_perp_metas(transport).await?;
    Dexes::new(list, metas).map_err(|error| ApiError::Unreadable { path: INFO, detail: error.to_string() })
}

/// Why `perpDexs` and `allPerpMetas` do not line up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Misaligned {
    /// The two reads list a different number of dexes.
    #[error("{dexes} dexes and {metas} metas, which should line up one for one")]
    Count {
        /// How many dexes `perpDexs` lists.
        dexes: usize,
        /// How many metas `allPerpMetas` lists.
        metas: usize,
    },
    /// A coin in a dex's meta that is not named after the dex.
    #[error("{coin} is listed under dex {dex}")]
    Coin {
        /// The coin as the meta lists it.
        coin: String,
        /// The dex whose meta lists it.
        dex: String,
    },
}

/// Every perp dex with its meta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dexes {
    /// Every dex as `perpDexs` lists it.
    pub list: PerpDexs,
    /// One per dex, in `perpDexs` order, the main dex's first.
    pub metas: Vec<Meta>,
}

impl Dexes {
    /// Lines `list` up with `metas`, refusing a count mismatch or a coin not named after its dex.
    pub fn new(list: PerpDexs, metas: Vec<Meta>) -> Result<Dexes, Misaligned> {
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
    pub fn dexes(&self) -> impl Iterator<Item = (u32, &PerpDex, &Meta)> {
        self.list.dexes().filter_map(|(place, dex)| Some((place, dex, self.metas.get(usize::try_from(place).ok()?)?)))
    }

    /// A HIP-3 dex by its name.
    #[must_use]
    pub fn dex(&self, name: &str) -> Option<(u32, &PerpDex, &Meta)> {
        self.dexes().find(|(_, dex, _)| dex.name == name)
    }

    /// A HIP-3 perp by its coin, `xyz:TSLA`, with its asset id.
    #[must_use]
    pub fn find(&self, coin: &str) -> Option<(Asset, &PerpAsset)> {
        let (name, _) = split_coin(coin)?;
        let (place, _, meta) = self.dex(name)?;
        meta.find(place, coin)
    }

    /// A HIP-3 perp by its asset id, with its dex.
    #[must_use]
    pub fn get(&self, asset: Asset) -> Option<(&PerpDex, &PerpAsset)> {
        let (place, _) = asset.hip3_parts()?;
        let (_, dex, meta) = self.dexes().find(|(at, _, _)| *at == place)?;
        Some((dex, meta.get(place, asset)?))
    }
}
