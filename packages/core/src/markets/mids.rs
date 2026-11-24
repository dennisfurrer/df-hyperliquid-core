//! `allMids`: every coin's mid on one perp dex, the same map from `/info` and from the socket.

use std::fmt;

use serde::{Deserialize, Deserializer, de};

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::dex::Dex;
use crate::utils::dec::Dec;

/// `{"type":"allMids","dex"?}`: the main dex, spot and outcomes with it, unless a HIP-3 dex is
/// named.
#[must_use]
pub fn all_mids_request(dex: Dex<'_>) -> String {
    info_request("allMids", dex.field().as_slice())
}

/// Every coin's mid on `dex`.
pub async fn all_mids(transport: &impl Transport, dex: Dex<'_>) -> Result<Mids, ApiError> {
    info(transport, all_mids_request(dex)).await
}

/// Every coin's mid on one dex, in the venue's order. The main dex's also carries spot pairs
/// (`@107`, `PURR/USDC`) and outcome sides (`#14730`). Where a book is empty the venue prints its
/// last trade instead, and a mid can sit between two ticks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mids(Vec<(String, Dec)>);

impl Mids {
    /// A coin's mid, the coin spelled as the venue spells it (`BTC`, `@107`, `#14730`).
    #[must_use]
    pub fn get(&self, coin: &str) -> Option<Dec> {
        self.0.iter().find(|(name, _)| name == coin).map(|(_, mid)| *mid)
    }

    /// How many coins have a mid.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no coin has a mid.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// In the order written.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Dec)> {
        self.0.iter().map(|(coin, mid)| (coin.as_str(), *mid))
    }
}

impl FromIterator<(String, Dec)> for Mids {
    fn from_iter<I: IntoIterator<Item = (String, Dec)>>(pairs: I) -> Self {
        Mids(pairs.into_iter().collect())
    }
}

/// Reads the JSON map into pairs to keep the order written, which a map type would lose.
impl<'de> Deserialize<'de> for Mids {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Pairs;

        impl<'de> de::Visitor<'de> for Pairs {
            type Value = Mids;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of coin to price")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(pair) = map.next_entry::<String, Dec>()? {
                    out.push(pair);
                }
                Ok(Mids(out))
            }
        }

        deserializer.deserialize_map(Pairs)
    }
}

#[cfg(test)]
#[path = "tests/mids.rs"]
mod tests;
