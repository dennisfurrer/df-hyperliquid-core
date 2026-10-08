//! The frames received, read into a type per channel ([`decode`]).
//!
//! Every frame is `{"channel":..,"data":..}`. The channel says what `data` is; it is read
//! where it lies in the text, into exact decimals, and nothing in it passes through a float. A
//! channel this build does not know is [`Frame::Other`], its data kept as written, never an
//! error; a frame that does not read is a [`BadFrame`], never a panic.

use std::{borrow::Cow, fmt};

use hl_core::{ExchangeResponse, Json};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::{
    account::{
        AllDexsState, AssetData, DexState, HistoricalOrders, Ledger, OpenOrders, OrderUpdate, SpotState, TwapHistory,
        TwapSliceFills, TwapStates, UserEvent, UserFills, UserFundings, WebData3,
    },
    market::{Bbo, Book, Candle, Mids, PerpCtx, SpotCtx, Trade},
    subscription::{BadSubscription, Subscription},
};

/// One frame, by channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// `pong`: the answer to a ping.
    Pong,
    /// `subscriptionResponse`: the venue took a subscribe or an unsubscribe.
    Ack(Ack),
    /// `error`: the venue refused something sent, in its own words.
    Error(Refusal),
    /// `post`: the answer to a post, by its id.
    Post(Reply),
    AllMids(Mids),
    /// `l2Book`.
    Book(Book),
    Trades(Vec<Trade>),
    Bbo(Bbo),
    Candle(Candle),
    /// `activeAssetCtx`: a perp's context.
    AssetCtx {
        coin: String,
        ctx: PerpCtx,
    },
    /// `activeSpotAssetCtx`: what `activeAssetCtx` sends for a spot pair or an outcome side.
    SpotAssetCtx {
        coin: String,
        ctx: SpotCtx,
    },
    /// `activeAssetData`.
    AssetData(AssetData),
    UserFills(UserFills),
    OrderUpdates(Vec<OrderUpdate>),
    /// `user`: what `userEvents` sends.
    UserEvent(UserEvent),
    UserFundings(UserFundings),
    /// `userNonFundingLedgerUpdates`.
    Ledger(Ledger),
    ClearinghouseState(DexState),
    AllDexsClearinghouseState(AllDexsState),
    OpenOrders(OpenOrders),
    SpotState(SpotState),
    TwapStates(TwapStates),
    /// `userTwapSliceFills`.
    TwapSliceFills(TwapSliceFills),
    /// `userTwapHistory`.
    TwapHistory(TwapHistory),
    Notification(String),
    WebData3(WebData3),
    /// Every perp dex's contexts, by dex name, the first `""`.
    AllDexsAssetCtxs(Vec<(String, Vec<PerpCtx>)>),
    AssetCtxs {
        dex: String,
        ctxs: Vec<PerpCtx>,
    },
    SpotAssetCtxs(Vec<SpotCtx>),
    /// What changed in `outcomeMeta`, kept as sent: `[{"outcomeSettled":23205}]`. Its arrival
    /// is the news; read `outcomeMeta` again for the whole.
    OutcomeMetaUpdates(Json),
    /// `fastAssetCtxs`: base64 of raw DEFLATE (RFC 1951) over JSON, kept as sent. Inflating it
    /// needs a decompressor this crate does not carry.
    FastAssetCtxs(String),
    /// `userHistoricalOrders`.
    HistoricalOrders(HistoricalOrders),
    /// A channel this build does not read: its name, and its data as written (empty when the
    /// frame has none).
    Other {
        channel: String,
        data: String,
    },
}

/// Which way a subscription went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Subscribe,
    Unsubscribe,
}

/// The venue's acknowledgement, echoing the subscription as it read it: keys reordered, left
/// out options filled in, the user in lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    pub method: Method,
    pub echo: Json,
}

impl Ack {
    /// The subscription it names, read as [`Subscription::parse`] reads one.
    pub fn subscription(&self) -> Result<Subscription, BadSubscription> {
        Subscription::parse(&self.echo)
    }
}

/// The venue refusing something: `Already subscribed: {...}`, `Already unsubscribed: {...}`,
/// `Error parsing JSON into valid websocket request: {...}` with what was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub message: String,
}

impl Refusal {
    /// The post it refuses: from an `id=<n>` at the end of the message, or the `id` of a post
    /// it quotes back.
    #[must_use]
    pub fn post_id(&self) -> Option<u64> {
        if let Some((_, digits)) = self.message.rsplit_once("id=")
            && !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return digits.parse().ok();
        }
        let quoted = self.quoted()?;
        match quoted.get("method").and_then(Json::as_str) {
            Some("post") => quoted.get("id").and_then(Json::as_u64),
            _ => None,
        }
    }

    /// The subscription it refuses, when it quotes one this crate reads.
    #[must_use]
    pub fn subscription(&self) -> Option<Subscription> {
        let quoted = self.quoted()?;
        let subscription = match quoted.get("subscription") {
            Some(inside) => inside,
            None if ["Already subscribed", "Already unsubscribed", "Invalid subscription"]
                .iter()
                .any(|start| self.message.starts_with(start)) =>
            {
                &quoted
            }
            None => return None,
        };
        Subscription::parse(subscription).ok()
    }

    /// The JSON the message ends with, when it ends with some.
    fn quoted(&self) -> Option<Json> {
        let at = self.message.find('{')?;
        Json::parse(self.message.get(at..)?).ok()
    }
}

/// The answer to a post, by the id it was sent with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: u64,
    pub answer: Answer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// What `/info` would have answered, as written (`null` for an unknown coin's book), for a
    /// read's own type to deserialize.
    Info { kind: String, data: String },
    /// What `/exchange` would have answered, read as hl-core reads it.
    Action(ExchangeResponse),
    /// The post was not taken: what HTTP would have said.
    Error(String),
}

/// A frame that did not read: the channel it named, when it got that far, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadFrame {
    pub channel: Option<String>,
    pub detail: String,
}

impl fmt::Display for BadFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.channel {
            Some(channel) => write!(f, "a `{channel}` frame did not read: {}", self.detail),
            None => write!(f, "not a frame: {}", self.detail),
        }
    }
}

impl std::error::Error for BadFrame {}

#[derive(Deserialize)]
struct Head<'a> {
    #[serde(borrow)]
    channel: Cow<'a, str>,
    #[serde(default, borrow)]
    data: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct RawAck<'a> {
    method: Method,
    #[serde(borrow)]
    subscription: &'a RawValue,
}

#[derive(Deserialize)]
struct RawReply<'a> {
    id: u64,
    #[serde(borrow)]
    response: RawResponse<'a>,
}

#[derive(Deserialize)]
struct RawResponse<'a> {
    #[serde(rename = "type", borrow)]
    kind: Cow<'a, str>,
    #[serde(borrow)]
    payload: &'a RawValue,
}

#[derive(Deserialize)]
struct RawInfo<'a> {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, borrow)]
    data: Option<&'a RawValue>,
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

/// Reads one text frame.
pub fn decode(text: &str) -> Result<Frame, BadFrame> {
    let head: Head<'_> = serde_json::from_str(text)
        .map_err(|error| BadFrame { channel: None, detail: format!("{error} in {}", shown(text)) })?;
    let channel = head.channel.as_ref();
    let data = head.data.map(RawValue::get);
    let typed = Typed { channel, data };
    Ok(match channel {
        "pong" => Frame::Pong,
        "subscriptionResponse" => {
            let ack: RawAck<'_> = typed.read()?;
            let echo = Json::parse(ack.subscription.get()).map_err(|error| typed.fail(error.to_string()))?;
            Frame::Ack(Ack { method: ack.method, echo })
        }
        "error" => Frame::Error(Refusal { message: typed.read()? }),
        "post" => Frame::Post(typed.reply()?),
        "allMids" => Frame::AllMids(typed.read()?),
        "l2Book" => Frame::Book(typed.read()?),
        "trades" => Frame::Trades(typed.read()?),
        "bbo" => Frame::Bbo(typed.read()?),
        "candle" => Frame::Candle(typed.read()?),
        "activeAssetCtx" => {
            let active: Active<PerpCtx> = typed.read()?;
            Frame::AssetCtx { coin: active.coin, ctx: active.ctx }
        }
        "activeSpotAssetCtx" => {
            let active: Active<SpotCtx> = typed.read()?;
            Frame::SpotAssetCtx { coin: active.coin, ctx: active.ctx }
        }
        "activeAssetData" => Frame::AssetData(typed.read()?),
        "userFills" => Frame::UserFills(typed.read()?),
        "orderUpdates" => Frame::OrderUpdates(typed.read()?),
        "user" => Frame::UserEvent(typed.read()?),
        "userFundings" => Frame::UserFundings(typed.read()?),
        "userNonFundingLedgerUpdates" => Frame::Ledger(typed.read()?),
        "clearinghouseState" => Frame::ClearinghouseState(typed.read()?),
        "allDexsClearinghouseState" => Frame::AllDexsClearinghouseState(typed.read()?),
        "openOrders" => Frame::OpenOrders(typed.read()?),
        "spotState" => Frame::SpotState(typed.read()?),
        "twapStates" => Frame::TwapStates(typed.read()?),
        "userTwapSliceFills" => Frame::TwapSliceFills(typed.read()?),
        "userTwapHistory" => Frame::TwapHistory(typed.read()?),
        "notification" => Frame::Notification(typed.read::<Note>()?.notification),
        "webData3" => Frame::WebData3(typed.read()?),
        "allDexsAssetCtxs" => Frame::AllDexsAssetCtxs(typed.read::<EveryDexCtxs>()?.ctxs),
        "assetCtxs" => {
            let dex: DexCtxs = typed.read()?;
            Frame::AssetCtxs { dex: dex.dex, ctxs: dex.ctxs }
        }
        "spotAssetCtxs" => Frame::SpotAssetCtxs(typed.read()?),
        "outcomeMetaUpdates" => {
            Frame::OutcomeMetaUpdates(Json::parse(typed.data()?).map_err(|error| typed.fail(error.to_string()))?)
        }
        "fastAssetCtxs" => Frame::FastAssetCtxs(typed.read()?),
        "userHistoricalOrders" => Frame::HistoricalOrders(typed.read()?),
        other => Frame::Other { channel: other.to_owned(), data: data.unwrap_or_default().to_owned() },
    })
}

/// A frame's data, read for the channel that named it.
struct Typed<'a> {
    channel: &'a str,
    data: Option<&'a str>,
}

impl<'a> Typed<'a> {
    fn fail(&self, detail: String) -> BadFrame {
        BadFrame { channel: Some(self.channel.to_owned()), detail }
    }

    fn data(&self) -> Result<&'a str, BadFrame> {
        self.data.ok_or_else(|| self.fail("no data".to_owned()))
    }

    fn read<T: Deserialize<'a>>(&self) -> Result<T, BadFrame> {
        let data = self.data()?;
        serde_json::from_str(data).map_err(|error| self.fail(format!("{error} in {}", shown(data))))
    }

    fn reply(&self) -> Result<Reply, BadFrame> {
        let data = self.data()?;
        let reply: RawReply<'_> =
            serde_json::from_str(data).map_err(|error| self.fail(format!("{error} in {}", shown(data))))?;
        let payload = reply.response.payload.get();
        let answer = match reply.response.kind.as_ref() {
            "info" => {
                let info: RawInfo<'_> = serde_json::from_str(payload).map_err(|error| self.fail(error.to_string()))?;
                Answer::Info { kind: info.kind, data: info.data.map_or("null", RawValue::get).to_owned() }
            }
            "action" => Answer::Action(ExchangeResponse::parse(payload).map_err(|error| self.fail(error.to_string()))?),
            "error" => Answer::Error(serde_json::from_str(payload).map_err(|error| self.fail(error.to_string()))?),
            other => return Err(self.fail(format!("a post answered as `{other}`"))),
        };
        Ok(Reply { id: reply.id, answer })
    }
}

/// The start of a text, for an error message.
fn shown(text: &str) -> String {
    let start: String = text.chars().take(160).collect();
    match start.len() < text.len() {
        true => format!("{start}..."),
        false => start,
    }
}

#[cfg(test)]
#[path = "tests/frame.rs"]
mod tests;
