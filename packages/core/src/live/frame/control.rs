//! The venue's word on what was sent: an [`Ack`] when it took a subscribe or an unsubscribe, a
//! [`Refusal`] in its own words when it refused something, a post included.

use crate::live::{BadSubscription, Subscription};
use crate::utils::Json;
use serde::Deserialize;

/// Which way a subscription went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// `subscribe`.
    Subscribe,
    /// `unsubscribe`.
    Unsubscribe,
}

/// The venue's acknowledgement, echoing the subscription as it read it: keys reordered, left
/// out options filled in, the user in lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    /// Whether a subscribe or an unsubscribe was taken.
    pub method: Method,
    /// `subscription`: the subscription as the venue echoed it.
    pub echo: Json,
}

impl Ack {
    /// The subscription it names.
    pub fn subscription(&self) -> Result<Subscription, BadSubscription> {
        Subscription::parse(&self.echo)
    }
}

/// The venue refusing something: `Already subscribed: {...}`, `Already unsubscribed: {...}`,
/// `Error parsing JSON into valid websocket request: {...}` with what was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The venue's message, as sent.
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

#[cfg(test)]
#[path = "tests/control.rs"]
mod tests;
