//! `cancel` and `cancelByCloid`. One action may name assets on several dexes:
//! the asset id says which.

use crate::trade::orders::Cloid;
use crate::utils::Asset;
use crate::utils::sign::{L1Action, Map, Value};

/// `cancel`: `{a, o}` per order, by asset and the venue's order id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancel {
    cancels: Vec<(Asset, u64)>,
}

impl Cancel {
    /// Cancels each `(asset, oid)`.
    #[must_use]
    pub fn new(cancels: Vec<(Asset, u64)>) -> Cancel {
        Cancel { cancels }
    }

    /// Cancels one order by its oid.
    #[must_use]
    pub fn one(asset: Asset, oid: u64) -> Cancel {
        Cancel::new(vec![(asset, oid)])
    }

    /// Each `(asset, oid)` it cancels, in the order sent.
    #[must_use]
    pub fn cancels(&self) -> &[(Asset, u64)] {
        &self.cancels
    }
}

impl L1Action for Cancel {
    fn action(&self) -> Map {
        Map::new().put("type", "cancel").put(
            "cancels",
            Value::array(self.cancels.iter().map(|&(asset, oid)| Map::new().put("a", asset).put("o", oid))),
        )
    }
}

/// `cancelByCloid`: `{asset, cloid}` per order, spelled out, unlike `cancel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelByCloid {
    cancels: Vec<(Asset, Cloid)>,
}

impl CancelByCloid {
    /// Cancels each `(asset, cloid)`.
    #[must_use]
    pub fn new(cancels: Vec<(Asset, Cloid)>) -> CancelByCloid {
        CancelByCloid { cancels }
    }

    /// Cancels one order by its client order id.
    #[must_use]
    pub fn one(asset: Asset, cloid: Cloid) -> CancelByCloid {
        CancelByCloid::new(vec![(asset, cloid)])
    }

    /// Each `(asset, cloid)` it cancels, in the order sent.
    #[must_use]
    pub fn cancels(&self) -> &[(Asset, Cloid)] {
        &self.cancels
    }
}

impl L1Action for CancelByCloid {
    fn action(&self) -> Map {
        Map::new().put("type", "cancelByCloid").put(
            "cancels",
            Value::array(
                self.cancels.iter().map(|&(asset, cloid)| Map::new().put("asset", asset).put("cloid", cloid.to_hex())),
            ),
        )
    }
}
