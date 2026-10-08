//! `l4Book`: every resting limit order of a few markets, with its owner, at one block height.
//! Hydromancer refreshes it every second; a stream starts from it ([`crate::Assembler`]).
//!
//! The answer is binary, little-endian:
//!
//! ```text
//! [count: u32][height: u64][timestamp_ms: u64]
//! count times: [blob_len: u32][zstd(msgpack([coin, bids, asks]))]
//! an order: [user, px, sz, oid], px and sz decimal strings
//! ```
//!
//! Every length is checked before it is read, and nothing is reserved on a length's word: a cut
//! or lying frame is refused, never read past. A blob opens to one zstd frame, whole, its
//! checksum checked where it carries one. An order is read by position, so a short one is
//! refused rather than guessed at (user and price are both strings: a shifted tuple cannot be
//! told by type).

use std::{fmt, io::Read as _};

use hl_core::{Address, Dec, Http, Json};
use serde::{
    Deserialize, Deserializer,
    de::{self, IgnoredAny, SeqAccess, Visitor},
};

use crate::client::{Call, Error, Hydromancer};

/// The most one market's blob may open to. A busy perp's whole book is a few megabytes.
pub const MAX_MARKET_BYTES: u64 = 64 * 1024 * 1024;

/// Which side of the book an order rests on: `B` (a bid) and `A` (an ask) on the wire.
/// hl-core's, the one an order is placed with.
pub use hl_core::Side;

/// One resting order and its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub oid: u64,
    pub user: Address,
    pub side: Side,
    pub px: Dec,
    pub sz: Dec,
}

/// One market's book, each side in the order the provider listed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Market {
    pub coin: String,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

/// Every market asked for, at one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The block whose changes the books hold: a stream applies blocks above it.
    pub height: u64,
    /// Unix milliseconds.
    pub time: u64,
    pub markets: Vec<Market>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("L4 snapshot: {0}")]
pub struct BadSnapshot(pub String);

/// `{"type":"l4Book","coins":[...]}`; no coins is every perp. A coin may be a perp, a spot pair
/// or an outcome side (`#2870`).
#[must_use]
pub fn l4_book(coins: &[&str]) -> Call {
    let mut body = vec![("type".to_owned(), Json::String("l4Book".to_owned()))];
    if !coins.is_empty() {
        body.push((
            "coins".to_owned(),
            Json::Array(coins.iter().map(|coin| Json::String((*coin).to_owned())).collect()),
        ));
    }
    Call::info(&Json::Object(body))
}

impl<H: Http> Hydromancer<H> {
    /// The L4 book of `coins` now. Hydromancer allows ten of these every five minutes and
    /// answers 404 until it has a snapshot to give.
    pub async fn l4_snapshot(&self, coins: &[&str]) -> Result<Snapshot, Error> {
        let call = l4_book(coins);
        let response = self.send(&call).await?;
        decode(&response.body).map_err(|BadSnapshot(detail)| self.unreadable(&call, &detail))
    }
}

/// Reads a whole `l4Book` frame.
pub fn decode(data: &[u8]) -> Result<Snapshot, BadSnapshot> {
    let mut reader = Reader { data, at: 0 };
    let count = reader.u32("the market count")?;
    let height = reader.u64("the height")?;
    let time = reader.u64("the timestamp")?;
    let mut markets = Vec::new();
    for index in 0..count {
        let len = reader.u32(&format!("market {index}'s length"))?;
        let blob = reader.take(usize::try_from(len).unwrap_or(usize::MAX), &format!("market {index}'s blob"))?;
        let opened = unzstd(blob).map_err(|detail| BadSnapshot(format!("market {index}: {detail}")))?;
        let raw: RawMarket =
            rmp_serde::from_slice(&opened).map_err(|error| BadSnapshot(format!("market {index}: {error}")))?;
        markets.push(raw.read(index)?);
    }
    if reader.at != data.len() {
        return Err(BadSnapshot(format!("{} bytes after the last of {count} markets", data.len() - reader.at)));
    }
    Ok(Snapshot { height, time, markets })
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize, what: &str) -> Result<&'a [u8], BadSnapshot> {
        let end = self.at.checked_add(len).filter(|&end| end <= self.data.len());
        let Some(bytes) = end.and_then(|end| self.data.get(self.at..end)) else {
            return Err(BadSnapshot(format!(
                "cut short: {what} needs {len} bytes at offset {}, and the frame is {}",
                self.at,
                self.data.len()
            )));
        };
        self.at += len;
        Ok(bytes)
    }

    fn u32(&mut self, what: &str) -> Result<u32, BadSnapshot> {
        let bytes = self.take(4, what)?;
        Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| BadSnapshot(format!("{what} is not four bytes")))?))
    }

    fn u64(&mut self, what: &str) -> Result<u64, BadSnapshot> {
        let bytes = self.take(8, what)?;
        Ok(u64::from_le_bytes(bytes.try_into().map_err(|_| BadSnapshot(format!("{what} is not eight bytes")))?))
    }
}

/// One zstd frame, whole: no bytes after it, no more than [`MAX_MARKET_BYTES`] out of it, and its
/// checksum matching where it carries one.
fn unzstd(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(blob).map_err(|error| format!("zstd: {error}"))?;
    let mut out = Vec::new();
    (&mut decoder).take(MAX_MARKET_BYTES + 1).read_to_end(&mut out).map_err(|error| format!("zstd: {error}"))?;
    if u64::try_from(out.len()).unwrap_or(u64::MAX) > MAX_MARKET_BYTES {
        return Err(format!("opens to more than {MAX_MARKET_BYTES} bytes"));
    }
    let frame = &decoder.decoder;
    if let Some(sent) = frame.get_checksum_from_data()
        && frame.get_calculated_checksum() != Some(sent)
    {
        return Err("zstd: the checksum does not match".to_owned());
    }
    if !decoder.get_ref().is_empty() {
        return Err(format!("{} bytes after the zstd frame", decoder.get_ref().len()));
    }
    Ok(out)
}

/// `[coin, bids, asks]`, anything after the third ignored.
struct RawMarket {
    coin: String,
    bids: Vec<RawOrder>,
    asks: Vec<RawOrder>,
}

/// `[user, px, sz, oid]`, anything after the fourth ignored.
struct RawOrder {
    user: String,
    px: String,
    sz: String,
    oid: u64,
}

impl RawMarket {
    fn read(self, index: u32) -> Result<Market, BadSnapshot> {
        let coin = self.coin;
        let orders = |raw: Vec<RawOrder>, side: Side, name: &str| -> Result<Vec<Order>, BadSnapshot> {
            raw.into_iter()
                .enumerate()
                .map(|(at, raw)| {
                    let bad = |what: &str, said: &str| BadSnapshot(format!("{coin} {name}[{at}]: {what} `{said}`"));
                    Ok(Order {
                        oid: raw.oid,
                        user: raw.user.parse().map_err(|_| bad("user is no address", &raw.user))?,
                        side,
                        px: Dec::parse(&raw.px).map_err(|_| bad("price is no decimal", &raw.px))?,
                        sz: Dec::parse(&raw.sz).map_err(|_| bad("size is no decimal", &raw.sz))?,
                    })
                })
                .collect()
        };
        let bids = orders(self.bids, Side::Buy, "bids")
            .map_err(|BadSnapshot(said)| BadSnapshot(format!("market {index}: {said}")))?;
        let asks = orders(self.asks, Side::Sell, "asks")
            .map_err(|BadSnapshot(said)| BadSnapshot(format!("market {index}: {said}")))?;
        Ok(Market { coin, bids, asks })
    }
}

/// The next element of a positional tuple, or why there is none.
fn next<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(seq: &mut A, at: usize, shape: &str) -> Result<T, A::Error> {
    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(at, &shape))
}

/// Steps over whatever a newer provider added after the fields read here.
fn rest<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

impl<'de> Deserialize<'de> for RawMarket {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Tuple;

        impl<'de> Visitor<'de> for Tuple {
            type Value = RawMarket;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("[coin, bids, asks]")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawMarket, A::Error> {
                let shape = "[coin, bids, asks]";
                let market = RawMarket {
                    coin: next(&mut seq, 0, shape)?,
                    bids: next(&mut seq, 1, shape)?,
                    asks: next(&mut seq, 2, shape)?,
                };
                rest(&mut seq)?;
                Ok(market)
            }
        }

        deserializer.deserialize_seq(Tuple)
    }
}

impl<'de> Deserialize<'de> for RawOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Tuple;

        impl<'de> Visitor<'de> for Tuple {
            type Value = RawOrder;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("[user, px, sz, oid]")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawOrder, A::Error> {
                let shape = "[user, px, sz, oid]";
                let order = RawOrder {
                    user: next(&mut seq, 0, shape)?,
                    px: next(&mut seq, 1, shape)?,
                    sz: next(&mut seq, 2, shape)?,
                    oid: next(&mut seq, 3, shape)?,
                };
                rest(&mut seq)?;
                Ok(order)
            }
        }

        deserializer.deserialize_seq(Tuple)
    }
}

#[cfg(test)]
#[path = "tests/snapshot.rs"]
mod tests;
