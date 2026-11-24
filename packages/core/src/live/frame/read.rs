//! Reading one text frame ([`decode`]): its channel names the type its data reads into. A frame
//! that does not read is a [`BadFrame`], naming the channel when it got that far.

use std::{borrow::Cow, fmt};

use crate::client::ExchangeResponse;
use crate::live::{Ack, Answer, Frame, Method, Refusal, Reply};
use crate::markets::context::{PerpCtx, SpotCtx};
use crate::utils::Json;
use serde::Deserialize;
use serde_json::value::RawValue;

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
        "l2" => Frame::L2(typed.read()?),
        "pac" => Frame::Pac(typed.read()?),
        "sac" => Frame::Sac(typed.read()?),
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
#[path = "tests/read.rs"]
mod tests;
