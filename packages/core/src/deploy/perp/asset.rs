//! What registering a HIP-3 perp carries: the asset, its margin mode, and the new dex's schema when
//! the first asset creates the dex.

use crate::utils::sign::{Map, Value};
use crate::utils::{Address, Dec};

/// How an asset may be margined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarginMode {
    /// `strictIsolated`: isolated only, and isolated margin cannot be taken
    /// out of an open position.
    StrictIsolated,
    /// `noCross`: isolated only.
    NoCross,
    /// `normal`: cross or isolated.
    Normal,
}

impl MarginMode {
    /// The name sent.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            MarginMode::StrictIsolated => "strictIsolated",
            MarginMode::NoCross => "noCross",
            MarginMode::Normal => "normal",
        }
    }
}

/// A new asset's listing: its coin (`dex:NAME`), size decimals, starting
/// oracle price and margin table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAsset {
    /// The coin, `dex:NAME`.
    pub coin: String,
    /// Size decimals (`szDecimals`): how many decimal places an order size may have.
    pub sz_decimals: u32,
    /// The starting oracle price (`oraclePx`).
    pub oracle_px: Dec,
    /// The id of the margin table the asset uses (`marginTableId`).
    pub margin_table_id: u32,
}

impl NewAsset {
    /// `{"coin","szDecimals","oraclePx","marginTableId"}`, the margin key
    /// for the caller to add last.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new()
            .put("coin", self.coin.clone())
            .put("szDecimals", self.sz_decimals)
            .put("oraclePx", self.oracle_px.to_wire())
            .put("marginTableId", self.margin_table_id)
    }
}

/// A new dex, registered with its first asset: its full name, the spot
/// token it is collateralised in, and who updates its oracle (`None` for the
/// deployer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DexSchema {
    /// The dex's full name (`fullName`).
    pub full_name: String,
    /// The spot token index of the collateral (`collateralToken`).
    pub collateral_token: u32,
    /// The address that updates the oracle (`oracleUpdater`); `None`, sent as null, for the deployer.
    pub oracle_updater: Option<Address>,
}

impl DexSchema {
    /// `{"fullName","collateralToken","oracleUpdater"}`; no oracle updater is
    /// sent as null.
    #[must_use]
    pub fn wire(&self) -> Map {
        Map::new()
            .put("fullName", self.full_name.clone())
            .put("collateralToken", self.collateral_token)
            .put("oracleUpdater", self.oracle_updater.map_or(Value::Nil, |updater| updater.to_hex().into()))
    }
}
