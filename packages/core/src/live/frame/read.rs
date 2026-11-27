//! Reading one text frame ([`decode`]): its channel names the type its data reads into. A frame
//! that does not read is a [`BadFrame`], naming the channel when it got that far.

use std::{borrow::Cow, fmt};

use crate::client::ExchangeResponse;
use crate::live::{Ack, Answer, Frame, Method, Reply};
use crate::utils::Json;
use serde::Deserialize;
use serde_json::value::RawValue;

use super::one_pass::{channel_of, decode_in_one, is_plain, plain};

/// A frame that did not read: the channel it named, when it got that far, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BadFrame {
    /// The channel the frame named; `None` when it did not get that far.
    pub channel: Option<String>,
    /// Why it did not read.
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

/// Reads one text frame. `l2Book`, `trades` and `bbo` in the venue's own compact layout are read
/// straight from the text. Other frames are read in one pass when they are
/// `{"channel":..,"data":..}` in that order with nothing else, as the venue writes every data frame.
/// Anything else, and every frame that does not read, is read in two passes (the frame with its
/// data kept as written, then the data), which gives the same frame or says why not.
pub fn decode(text: &str) -> Result<Frame, BadFrame> {
    if let Some(frame) = super::hot::read(text) {
        return Ok(frame);
    }
    if channel_of(text).is_some_and(is_plain)
        && let Ok(frame) = decode_in_one(text)
    {
        return Ok(frame);
    }
    decode_in_two(text)
}

/// Whether [`decode`] reads `text` with its direct reader: an `l2Book`, `trades` or `bbo` frame in
/// the venue's compact layout. For the live suite, which checks that the venue still writes that
/// layout: behind the `__test-internals` feature, outside the crate's API and its semver promise.
/// It reads the frame again, so it is not for every frame of a running program.
#[cfg(feature = "__test-internals")]
#[doc(hidden)]
#[must_use]
pub fn reads_directly(text: &str) -> bool {
    super::hot::read(text).is_some()
}

/// Two passes: the frame with its data kept as written, then the data read for its channel. Reads
/// every frame [`decode`] takes and names the reason for one it does not.
pub(crate) fn decode_in_two(text: &str) -> Result<Frame, BadFrame> {
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
        "post" => Frame::Post(typed.reply()?),
        "outcomeMetaUpdates" => {
            Frame::OutcomeMetaUpdates(Json::parse(typed.data()?).map_err(|error| typed.fail(error.to_string()))?)
        }
        other => match typed.plain() {
            Some(frame) => frame?,
            None => Frame::Other { channel: other.to_owned(), data: data.unwrap_or_default().to_owned() },
        },
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

    /// The data of a channel [`plain`] knows, read as `serde_json::from_str` would; `None` for
    /// any other channel.
    fn plain(&self) -> Option<Result<Frame, BadFrame>> {
        if !is_plain(self.channel) {
            return None;
        }
        let data = match self.data() {
            Ok(data) => data,
            Err(error) => return Some(Err(error)),
        };
        let mut de = serde_json::Deserializer::from_str(data);
        let frame = plain(self.channel, &mut de)?.and_then(|frame| de.end().map(|()| frame));
        Some(frame.map_err(|error| self.fail(format!("{error} in {}", shown(data)))))
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
