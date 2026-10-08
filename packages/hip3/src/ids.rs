//! A HIP-3 perp's two names: its coin, `dex:SYMBOL`, and its asset id,
//! `100000 + dex * 10000 + index`. `hl_core::Asset::hip3` makes an id; these
//! take one apart and split a coin.

use hl_core::{Asset, AssetKind, HIP3_ASSET_BASE, HIP3_DEX_SPAN};

/// `xyz:TSLA` as `("xyz", "TSLA")`. `None` for a main-dex coin, which has no
/// dex, or either part empty.
#[must_use]
pub fn split_coin(coin: &str) -> Option<(&str, &str)> {
    let (dex, symbol) = coin.split_once(':')?;
    (!dex.is_empty() && !symbol.is_empty()).then_some((dex, symbol))
}

/// `xyz` and `TSLA` as `xyz:TSLA`.
#[must_use]
pub fn coin(dex: &str, symbol: &str) -> String {
    format!("{dex}:{symbol}")
}

/// A HIP-3 perp's dex place in `perpDexs` and its index in that dex's meta.
/// `None` for any other kind of id.
#[must_use]
pub fn asset_parts(asset: Asset) -> Option<(u32, u32)> {
    if asset.kind() != AssetKind::Hip3Perp {
        return None;
    }
    let offset = asset.id().checked_sub(HIP3_ASSET_BASE)?;
    let place = offset / HIP3_DEX_SPAN;
    (place > 0).then_some((place, offset % HIP3_DEX_SPAN))
}
