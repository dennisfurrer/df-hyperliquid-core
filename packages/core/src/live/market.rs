//! The websocket's own market frames: the best bid and offer, and one dex's mids. Books, trades,
//! candles and asset contexts come in the shapes the `/info` reads answer, the [`crate::markets`]
//! types. Every price and size is an exact [`Dec`] read from the venue's string; a number written
//! as a float is refused, never rounded.

use crate::markets::book::Level;
use crate::utils::Dec;
use serde::Deserialize;

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
