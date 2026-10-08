//! A print: one trade on a market's tape, the same shape whether `recentTrades` answers it or
//! the `trades` feed sends it.

use serde::Deserialize;

use crate::{address::Address, dec::Dec, side::Side};

/// One print. `side` is the aggressor's.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "Raw")]
pub struct Trade {
    pub coin: String,
    /// The aggressor's side.
    pub side: Side,
    pub px: Dec,
    pub sz: Dec,
    /// Milliseconds.
    pub time: u64,
    /// The L1 transaction hash; all zeros for a TWAP slice.
    pub hash: String,
    /// A hash of the two order ids, not unique alone: with `time` and `coin` it is.
    pub tid: u64,
    pub buyer: Address,
    pub seller: Address,
}

impl Trade {
    /// `users` as the venue writes them: the buyer, then the seller.
    #[must_use]
    pub const fn users(&self) -> [Address; 2] {
        [self.buyer, self.seller]
    }
}

#[derive(Deserialize)]
struct Raw {
    coin: String,
    side: Side,
    px: Dec,
    sz: Dec,
    time: u64,
    hash: String,
    tid: u64,
    users: [Address; 2],
}

impl From<Raw> for Trade {
    fn from(raw: Raw) -> Self {
        let [buyer, seller] = raw.users;
        Trade {
            coin: raw.coin,
            side: raw.side,
            px: raw.px,
            sz: raw.sz,
            time: raw.time,
            hash: raw.hash,
            tid: raw.tid,
            buyer,
            seller,
        }
    }
}

#[cfg(test)]
#[path = "tests/trade.rs"]
mod tests;
