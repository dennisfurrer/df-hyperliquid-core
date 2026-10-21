//! Asset ids: the integer an order names its market by. Ids run perps on the
//! main dex below 10000, spot pairs from 10000, HIP-3 perps from 100000 and
//! HIP-4 outcomes from 100000000, so an id alone says what it names, and so
//! which builder cap holds it.

use std::fmt;

use crate::trade::builder::fee::{BuilderFee, OUTCOME_BUILDER_CAP, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
use crate::utils::sign::msgpack::Value;

/// Where spot pairs begin: `10000 + pair index`.
pub const SPOT_ASSET_BASE: u32 = 10_000;

/// Where HIP-3 perps begin: `100000 + dex * 10000 + index`, `dex` the dex's
/// place in `perpDexs`, where the main dex is 0.
pub const HIP3_ASSET_BASE: u32 = 100_000;

/// How many ids each HIP-3 dex spans.
pub const HIP3_DEX_SPAN: u32 = 10_000;

/// Where HIP-4 outcomes begin: `100000000 + 10 * outcome + side`.
pub const OUTCOME_ASSET_BASE: u32 = 100_000_000;

/// A HIP-4 outcome's side, the last digit of its encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutcomeSide {
    /// The first side, `0`: the one a settle fraction pays.
    Yes,
    No,
}

impl OutcomeSide {
    #[must_use]
    pub const fn digit(self) -> u32 {
        match self {
            OutcomeSide::Yes => 0,
            OutcomeSide::No => 1,
        }
    }

    #[must_use]
    pub const fn other(self) -> OutcomeSide {
        match self {
            OutcomeSide::Yes => OutcomeSide::No,
            OutcomeSide::No => OutcomeSide::Yes,
        }
    }

    /// `yes` or `no`, as serde writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            OutcomeSide::Yes => "yes",
            OutcomeSide::No => "no",
        }
    }
}

impl serde::Serialize for OutcomeSide {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for OutcomeSide {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "yes" => Ok(OutcomeSide::Yes),
            "no" => Ok(OutcomeSide::No),
            other => Err(serde::de::Error::custom(format!("`{other}` is not an outcome side; expected yes or no"))),
        }
    }
}

/// What an asset id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    /// A perp on the main dex: its index in `meta.universe`.
    Perp,
    /// A spot pair: `10000 + pair index`.
    Spot,
    /// A perp on a HIP-3 dex: `100000 + dex * 10000 + index`.
    Hip3Perp,
    /// A HIP-4 outcome side: `100000000 + 10 * outcome + side`.
    Outcome,
}

impl AssetKind {
    pub const ALL: [AssetKind; 4] = [AssetKind::Perp, AssetKind::Spot, AssetKind::Hip3Perp, AssetKind::Outcome];

    #[must_use]
    pub const fn of(id: u32) -> AssetKind {
        if id >= OUTCOME_ASSET_BASE {
            AssetKind::Outcome
        } else if id >= HIP3_ASSET_BASE {
            AssetKind::Hip3Perp
        } else if id >= SPOT_ASSET_BASE {
            AssetKind::Spot
        } else {
            AssetKind::Perp
        }
    }

    /// The most a builder may take on an order of this kind: 0.1% on a perp,
    /// main dex or HIP-3, and 1% on spot. HIP-4 takes builder codes as spot
    /// does, per Hyperliquid's HIP-4 page.
    #[must_use]
    pub const fn builder_cap(self) -> BuilderFee {
        match self {
            AssetKind::Perp | AssetKind::Hip3Perp => PERP_BUILDER_CAP,
            AssetKind::Spot => SPOT_BUILDER_CAP,
            AssetKind::Outcome => OUTCOME_BUILDER_CAP,
        }
    }

    /// The kind that holds an action over these assets to its builder cap:
    /// the one with the lowest, so one perp leg holds the whole action to the
    /// perp cap. `None` for no assets.
    #[must_use]
    pub fn strictest(assets: impl IntoIterator<Item = Asset>) -> Option<AssetKind> {
        assets.into_iter().map(Asset::kind).reduce(|kept, next| match next.builder_cap() < kept.builder_cap() {
            true => next,
            false => kept,
        })
    }

    /// `a perp`, `an outcome`: one order of this kind, in a sentence.
    #[must_use]
    pub const fn one(self) -> &'static str {
        match self {
            AssetKind::Perp => "a perp",
            AssetKind::Spot => "a spot",
            AssetKind::Hip3Perp => "a HIP-3 perp",
            AssetKind::Outcome => "an outcome",
        }
    }
}

/// An asset id. Made from the index it is counted from, so a pair index is
/// never sent where an asset id belongs; an index past its range is refused
/// rather than spilling into the next kind's ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Asset(u32);

impl Asset {
    /// An id as the venue wrote it, in an order status or a fill.
    #[must_use]
    pub const fn from_id(id: u32) -> Asset {
        Asset(id)
    }

    #[must_use]
    pub const fn id(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn kind(self) -> AssetKind {
        AssetKind::of(self.0)
    }

    /// A perp on the main dex, by its index in `meta.universe`.
    #[must_use]
    pub const fn perp(index: u32) -> Option<Asset> {
        match index < SPOT_ASSET_BASE {
            true => Some(Asset(index)),
            false => None,
        }
    }

    /// A spot pair, by the pair's own `index` in `spotMeta.universe`, which
    /// is not a token's index.
    #[must_use]
    pub const fn spot(pair: u32) -> Option<Asset> {
        match pair < HIP3_ASSET_BASE - SPOT_ASSET_BASE {
            true => Some(Asset(SPOT_ASSET_BASE + pair)),
            false => None,
        }
    }

    /// A perp on a HIP-3 dex: `dex` its place in `perpDexs` (1 or more; the
    /// main dex at 0 is [`Asset::perp`]), `index` its place in that dex's
    /// `meta.universe`.
    #[must_use]
    pub const fn hip3(dex: u32, index: u32) -> Option<Asset> {
        if dex == 0 || index >= HIP3_DEX_SPAN {
            return None;
        }
        let Some(offset) = dex.checked_mul(HIP3_DEX_SPAN) else {
            return None;
        };
        let Some(start) = HIP3_ASSET_BASE.checked_add(offset) else {
            return None;
        };
        match start.checked_add(index) {
            Some(id) if id < OUTCOME_ASSET_BASE => Some(Asset(id)),
            _ => None,
        }
    }

    /// A HIP-3 perp's dex place in `perpDexs` and its index in that dex's
    /// `meta`: [`Asset::hip3`] taken apart. `None` for any other kind of id.
    #[must_use]
    pub const fn hip3_parts(self) -> Option<(u32, u32)> {
        if !matches!(self.kind(), AssetKind::Hip3Perp) {
            return None;
        }
        let offset = self.0 - HIP3_ASSET_BASE;
        match offset / HIP3_DEX_SPAN {
            0 => None,
            place => Some((place, offset % HIP3_DEX_SPAN)),
        }
    }

    /// A perp by its dex's place in `perpDexs`: the main dex at 0, else HIP-3.
    #[must_use]
    pub const fn perp_on(dex: u32, index: u32) -> Option<Asset> {
        match dex {
            0 => Asset::perp(index),
            dex => Asset::hip3(dex, index),
        }
    }

    /// A HIP-4 outcome side: `100000000 + 10 * outcome + side`.
    #[must_use]
    pub const fn outcome(outcome: u32, side: OutcomeSide) -> Option<Asset> {
        let Some(tens) = outcome.checked_mul(10) else {
            return None;
        };
        match OUTCOME_ASSET_BASE.checked_add(tens + side.digit()) {
            Some(id) => Some(Asset(id)),
            None => None,
        }
    }

    /// The outcome and side an outcome id names; `None` for any other kind.
    #[must_use]
    pub const fn outcome_side(self) -> Option<(u32, OutcomeSide)> {
        if self.0 < OUTCOME_ASSET_BASE {
            return None;
        }
        let encoding = self.0 - OUTCOME_ASSET_BASE;
        let side = match encoding % 10 {
            0 => OutcomeSide::Yes,
            1 => OutcomeSide::No,
            _ => return None,
        };
        Some((encoding / 10, side))
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An asset goes on the wire as its plain id.
impl From<Asset> for Value {
    fn from(asset: Asset) -> Value {
        Value::Uint(u64::from(asset.0))
    }
}

#[cfg(test)]
#[path = "tests/asset.rs"]
mod tests;
