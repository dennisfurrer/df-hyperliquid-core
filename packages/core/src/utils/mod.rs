//! Helpers every Hyperliquid app needs: exact decimals, price and size rounding, asset ids, units, time, hex, nonces and signing primitives.
pub mod address;
pub mod asset;
pub mod dec;
pub mod figure;
pub mod hex;
pub mod json;
pub mod nonce;
pub mod rounding;
pub mod sign;
pub mod time;
pub mod units;

pub use address::{Address, BadAddress};
pub use asset::{Asset, AssetKind, HIP3_ASSET_BASE, HIP3_DEX_SPAN, OUTCOME_ASSET_BASE, OutcomeSide, SPOT_ASSET_BASE};
pub use dec::{BadDec, Dec, Round};
pub use figure::Figure;
pub use hex::BadHex;
pub use json::Json;
pub use nonce::NonceClock;
pub use rounding::{
    Grid, MAX_SLIPPAGE, MINIMUM_ORDER_VALUE, PERP_PRICE_PLACES, SIGNIFICANT_FIGURES, SPOT_PRICE_PLACES,
};
pub use units::{BadUnits, cut_units, exact_units};
