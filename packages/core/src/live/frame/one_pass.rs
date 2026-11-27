//! Reading a frame in one pass: the channel, then its data read straight into the channel's type,
//! for the channels whose data is one type ([`plain`]). The two-pass reader in [`super::read`]
//! uses the same table, so both read each channel into the same type.

use std::{borrow::Cow, fmt};

use crate::live::{Frame, Refusal};
use crate::markets::context::{PerpCtx, SpotCtx};
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::scan::Scan;

/// One pass: the channel, then its data read straight into the channel's type. Only the channels
/// [`plain`] knows; any other channel, key or order is an error here. [`super::read::decode`] asks
/// [`is_plain`] first, so a frame of another channel never comes here.
pub(crate) fn decode_in_one(text: &str) -> Result<Frame, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(text);
    let frame = de.deserialize_map(InOne)?;
    de.end()?;
    Ok(frame)
}

struct InOne;

impl<'de> Visitor<'de> for InOne {
    type Value = Frame;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a channel, then its data")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Frame, A::Error> {
        let next = |key: Option<Cow<'de, str>>, want: &str| match key {
            Some(key) if key == want => Ok(()),
            _ => Err(de::Error::custom("not in one pass")),
        };
        next(map.next_key()?, "channel")?;
        let channel: Cow<'de, str> = map.next_value()?;
        next(map.next_key()?, "data")?;
        let frame = map.next_value_seed(Plain(&channel))?;
        match map.next_key::<IgnoredAny>()? {
            None => Ok(frame),
            Some(_) => Err(de::Error::custom("not in one pass")),
        }
    }
}

/// The data of a channel [`plain`] knows.
struct Plain<'a>(&'a str);

impl<'de> DeserializeSeed<'de> for Plain<'_> {
    type Value = Frame;

    fn deserialize<D: Deserializer<'de>>(self, data: D) -> Result<Frame, D::Error> {
        plain(self.0, data).unwrap_or_else(|| Err(de::Error::custom("not in one pass")))
    }
}

/// One list of the channels whose data reads straight into one type, and the type each reads
/// into: it makes both [`PLAIN`] and [`plain`], so the two cannot name different channels.
macro_rules! plain_channels {
    ($($channel:literal => $frame:expr,)*) => {
        /// The channels [`plain`] reads.
        const PLAIN: &[&str] = &[$($channel),*];

        /// The data of a channel in [`PLAIN`], read from `data`; `None` for any other channel.
        pub(super) fn plain<'de, D: Deserializer<'de>>(channel: &str, data: D) -> Option<Result<Frame, D::Error>> {
            Some(match channel {
                $($channel => read(data, $frame),)*
                _ => return None,
            })
        }
    };
}

plain_channels! {
    "error" => |message| Frame::Error(Refusal { message }),
    "allMids" => Frame::AllMids,
    "l2Book" => Frame::Book,
    "trades" => Frame::Trades,
    "bbo" => Frame::Bbo,
    "candle" => Frame::Candle,
    "activeAssetCtx" => |active: Active<PerpCtx>| Frame::AssetCtx { coin: active.coin, ctx: active.ctx },
    "activeSpotAssetCtx" => |active: Active<SpotCtx>| Frame::SpotAssetCtx { coin: active.coin, ctx: active.ctx },
    "activeAssetData" => Frame::AssetData,
    "userFills" => Frame::UserFills,
    "orderUpdates" => Frame::OrderUpdates,
    "user" => Frame::UserEvent,
    "userFundings" => Frame::UserFundings,
    "userNonFundingLedgerUpdates" => Frame::Ledger,
    "clearinghouseState" => Frame::ClearinghouseState,
    "allDexsClearinghouseState" => Frame::AllDexsClearinghouseState,
    "openOrders" => Frame::OpenOrders,
    "spotState" => Frame::SpotState,
    "twapStates" => Frame::TwapStates,
    "userTwapSliceFills" => Frame::TwapSliceFills,
    "userTwapHistory" => Frame::TwapHistory,
    "notification" => |note: Note| Frame::Notification(note.notification),
    "webData3" => Frame::WebData3,
    "allDexsAssetCtxs" => |every: EveryDexCtxs| Frame::AllDexsAssetCtxs(every.ctxs),
    "assetCtxs" => |dex: DexCtxs| Frame::AssetCtxs { dex: dex.dex, ctxs: dex.ctxs },
    "spotAssetCtxs" => Frame::SpotAssetCtxs,
    "fastAssetCtxs" => Frame::FastAssetCtxs,
    "userHistoricalOrders" => Frame::HistoricalOrders,
    "l2" => Frame::L2,
    "pac" => Frame::Pac,
    "sac" => Frame::Sac,
}

fn read<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    data: D,
    frame: impl FnOnce(T) -> Frame,
) -> Result<Frame, D::Error> {
    T::deserialize(data).map(frame)
}

/// Whether `channel`'s data reads straight into one type, so [`decode_in_one`] can read it.
pub(super) fn is_plain(channel: &str) -> bool {
    PLAIN.contains(&channel)
}

/// The channel of a frame that starts `{"channel":"..."`, read without going further.
pub(super) fn channel_of(text: &str) -> Option<&str> {
    let mut scan = Scan::new(text);
    scan.expect(r#"{"channel":"#)?;
    scan.string()
}

#[derive(Deserialize)]
struct Active<C> {
    coin: String,
    ctx: C,
}

#[derive(Deserialize)]
struct DexCtxs {
    dex: String,
    ctxs: Vec<PerpCtx>,
}

#[derive(Deserialize)]
struct EveryDexCtxs {
    ctxs: Vec<(String, Vec<PerpCtx>)>,
}

#[derive(Deserialize)]
struct Note {
    notification: String,
}

#[cfg(test)]
#[path = "tests/one_pass.rs"]
mod tests;
