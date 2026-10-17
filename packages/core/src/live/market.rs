//! Market data as the websocket sends it: books, the best bid and offer, trades, candles, mids
//! and asset contexts. Every price and size is an exact [`Dec`] read from the venue's string;
//! a number written as a float is refused, never rounded.

use crate::utils::Dec;
use serde::Deserialize;

/// One price level and `l2Book`, the whole visible book each frame, the shape the `/info` read
/// answers too.
pub use crate::markets::book::{Book, Level};

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
/// own, on an order the side it rests on.
pub use crate::markets::Side;

/// One print on `trades`, oldest first within a frame, the shape `recentTrades` answers too.
pub use crate::markets::trades::Trade;

/// `candle`: the bar of an interval as it stands, sent as it forms and then again for the next.
/// The shape `candleSnapshot` answers too.
pub use crate::markets::candle::Candle;

/// `allMids`: every coin's mid on one perp dex, in the order sent. The first dex's (`dex` empty)
/// also carries spot pairs (`@107`, `PURR/USDC`) and outcome sides (`#14720`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Mids {
    #[serde(default)]
    pub dex: String,
    /// The map `allMids` answers.
    pub mids: crate::markets::mids::Mids,
}

impl Mids {
    /// A coin's mid, the coin spelled as the venue spells it.
    #[must_use]
    pub fn get(&self, coin: &str) -> Option<Dec> {
        self.mids.get(coin)
    }
}

/// A perp's context, as `activeAssetCtx`, `assetCtxs` and `allDexsAssetCtxs` send it, and a spot
/// pair's or an outcome side's, as `activeSpotAssetCtx` and `spotAssetCtxs` send it, the shapes
/// `metaAndAssetCtxs` and `spotMetaAndAssetCtxs` answer.
pub use crate::markets::context::{PerpCtx, SpotCtx};
