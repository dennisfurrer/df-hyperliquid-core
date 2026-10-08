//! Market data as the websocket sends it: books, the best bid and offer, trades, candles, mids
//! and asset contexts. Every price and size is an exact [`Dec`] read from the venue's string;
//! a number written as a float is refused, never rounded.

use std::fmt;

use hl_core::Dec;
use serde::{Deserialize, Deserializer, de};

/// One price level and `l2Book`, the whole visible book each frame: hl-core's, the shape the
/// `/info` read answers too.
pub use hl_core::book::{Book, Level};

/// `bbo`: the best bid and offer, sent only when one of them moves. A side with nothing
/// resting is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawBbo")]
pub struct Bbo {
    pub coin: String,
    /// Milliseconds.
    pub time: u64,
    pub bid: Option<Level>,
    pub ask: Option<Level>,
}

#[derive(Deserialize)]
struct RawBbo {
    coin: String,
    time: u64,
    bbo: (Option<Level>, Option<Level>),
}

impl From<RawBbo> for Bbo {
    fn from(raw: RawBbo) -> Self {
        let (bid, ask) = raw.bbo;
        Bbo { coin: raw.coin, time: raw.time, bid, ask }
    }
}

/// `B` or `A`: a buy or a sell. On a trade it is the aggressor's side, on a fill the account's
/// own, on an order the side it rests on. hl-core's, the one an order is placed with.
pub use hl_core::Side;

/// One print on `trades`, oldest first within a frame: hl-core's, the shape `recentTrades`
/// answers too.
pub use hl_core::trade::Trade;

/// `candle`: the bar of an interval as it stands, sent as it forms and then again for the next.
/// hl-core's, the shape `candleSnapshot` answers too.
pub use hl_core::candle::Candle;

/// `allMids`: every coin's mid on one perp dex, in the order sent. The first dex's (`dex` empty)
/// also carries spot pairs (`@107`, `PURR/USDC`) and outcome sides (`#14720`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Mids {
    #[serde(default)]
    pub dex: String,
    #[serde(deserialize_with = "pairs")]
    pub mids: Vec<(String, Dec)>,
}

impl Mids {
    /// A coin's mid, the coin spelled as the venue spells it.
    #[must_use]
    pub fn get(&self, coin: &str) -> Option<Dec> {
        self.mids.iter().find(|(name, _)| name == coin).map(|(_, mid)| *mid)
    }
}

/// A JSON map read into pairs in the order written, which a map type would lose.
fn pairs<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<(String, Dec)>, D::Error> {
    struct Pairs;

    impl<'de> de::Visitor<'de> for Pairs {
        type Value = Vec<(String, Dec)>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map of coin to price")
        }

        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(pair) = map.next_entry::<String, Dec>()? {
                out.push(pair);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(Pairs)
}

/// A perp's context, as `activeAssetCtx`, `assetCtxs` and `allDexsAssetCtxs` send it and
/// `metaAndAssetCtxs` answers it. A market with no book has no mid, premium or impact prices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpCtx {
    /// Hourly, as a fraction of the position's value.
    pub funding: Dec,
    /// In the base asset.
    pub open_interest: Dec,
    pub prev_day_px: Dec,
    /// The quote traded over the last day.
    pub day_ntl_vlm: Dec,
    #[serde(default)]
    pub premium: Option<Dec>,
    pub oracle_px: Dec,
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    #[serde(default)]
    pub impact_pxs: Option<Vec<Dec>>,
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
}

/// A spot pair's or an outcome side's context, as `activeSpotAssetCtx` and `spotAssetCtxs` send
/// it. An outcome side's open interest is its `circulatingSupply`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotCtx {
    pub coin: String,
    pub prev_day_px: Dec,
    pub day_ntl_vlm: Dec,
    pub mark_px: Dec,
    #[serde(default)]
    pub mid_px: Option<Dec>,
    pub circulating_supply: Dec,
    #[serde(default)]
    pub total_supply: Option<Dec>,
    #[serde(default)]
    pub day_base_vlm: Option<Dec>,
}
