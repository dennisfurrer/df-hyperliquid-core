//! Which perp dex a read or a coin is about: a request without `dex` is the main dex's, one with
//! `"dex":"xyz"` that HIP-3 dex's. A HIP-3 coin is `dex:SYMBOL`; its asset id is
//! [`Asset::hip3`](crate::utils::asset::Asset::hip3).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Dex<'a> {
    /// The main dex, the first in `perpDexs`.
    #[default]
    Main,
    /// A HIP-3 dex by its name: `xyz`, the `xyz:` of its coins.
    Hip3(&'a str),
}

impl<'a> Dex<'a> {
    /// The dex a coin trades on: `xyz:TSLA` on `xyz`, `BTC` on the main dex.
    #[must_use]
    pub fn of_coin(coin: &'a str) -> Dex<'a> {
        match coin.split_once(':') {
            Some((dex, _)) => Dex::Hip3(dex),
            None => Dex::Main,
        }
    }

    /// The `dex` a request names; `None` for the main dex.
    #[must_use]
    pub const fn name(self) -> Option<&'a str> {
        match self {
            Dex::Main => None,
            Dex::Hip3(name) => Some(name),
        }
    }
}

/// `xyz:TSLA` as `("xyz", "TSLA")`. `None` for a main-dex coin or an empty part.
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

#[cfg(test)]
#[path = "tests/dex.rs"]
mod tests;
