//! A HIP-3 perp's two names: its coin, `dex:SYMBOL`, and its asset id,
//! `100000 + dex * 10000 + index`. `hl_core::Asset::hip3` makes an id and
//! `hl_core::Asset::hip3_parts` takes one apart; the coin split is hl-core's
//! too, shared with hl-perps.

use hl_core::Asset;

/// The dex a coin trades on, and `xyz:TSLA` as `("xyz", "TSLA")` and back.
pub use hl_core::dex::{Dex, coin, split_coin};

/// A HIP-3 perp's dex place in `perpDexs` and its index in that dex's meta.
/// `None` for any other kind of id.
#[must_use]
pub const fn asset_parts(asset: Asset) -> Option<(u32, u32)> {
    asset.hip3_parts()
}
