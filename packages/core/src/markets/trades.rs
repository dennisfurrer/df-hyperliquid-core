//! A print: one trade on a market's tape, the same shape from the `recentTrades` read and from the
//! `trades` feed.

use serde::Deserialize;

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::side::Side;
use crate::utils::Json;
use crate::utils::address::Address;
use crate::utils::dec::Dec;

/// `{"type":"recentTrades","coin":..}`.
#[must_use]
pub fn recent_trades_request(coin: &str) -> String {
    info_request("recentTrades", &[("coin", Json::String(coin.to_owned()))])
}

/// The coin's most recent trades.
pub async fn recent_trades(transport: &impl Transport, coin: &str) -> Result<Vec<Trade>, ApiError> {
    info(transport, recent_trades_request(coin)).await
}

/// One trade on a market's tape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "Raw")]
pub struct Trade {
    /// The coin traded.
    pub coin: String,
    /// The aggressor's side.
    pub side: Side,
    /// The trade price.
    pub px: Dec,
    /// The size traded, in the base asset.
    pub sz: Dec,
    /// Milliseconds.
    pub time: u64,
    /// The L1 transaction hash; all zeros for a TWAP slice.
    pub hash: String,
    /// A hash of the two order ids; unique only together with `time` and `coin`.
    pub tid: u64,
    /// The buying account, first in `users`.
    pub buyer: Address,
    /// The selling account, second in `users`.
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
#[path = "tests/trades.rs"]
mod tests;
