//! The [`Transport`] every read and action goes through, the `/info` bodies,
//! and the `/info` and `/exchange` calls themselves: a read once, a signed
//! action once and never again.

use std::future::Future;

use serde::de::DeserializeOwned;

use crate::client::http::transport::{Accepted, ExchangeResponse, TransportError};
use crate::utils::address::Address;
use crate::utils::json::Json;

/// The path every read is posted to.
pub const INFO: &str = "/info";
/// The path every signed action is posted to.
pub const EXCHANGE: &str = "/exchange";

/// Posts a JSON body to `path` (`/info` or `/exchange`) on one network and
/// returns the answer's text. [`crate::client::Client`] implements it behind
/// the rate limit. No `Send` bound, since the browser is single-threaded.
pub trait Transport {
    /// Posts `body` to `path` and returns the answer's text.
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>>;
}

/// Why a read or action through a [`Transport`] gave no usable answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ApiError {
    /// The request got no answer, a non-2xx status, or was rate limited.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The answer came back but did not parse as expected.
    #[error("the venue's answer on {path} could not be read: {detail}")]
    Unreadable {
        /// `/info` or `/exchange`.
        path: &'static str,
        /// The parse error and up to 300 characters of the answer.
        detail: String,
    },
    /// `{"status":"err"}`: the venue's own words, shown as they came.
    #[error("{0}")]
    Refused(String),
}

/// `{"type":<kind>, ..fields}`, the body of every `/info` read: `type` first,
/// then each field in the order given, so a request matches a recorded one
/// byte for byte.
#[must_use]
pub fn info_request(kind: &str, fields: &[(&str, Json)]) -> String {
    let mut object = Vec::with_capacity(fields.len() + 1);
    object.push(("type".to_owned(), Json::String(kind.to_owned())));
    object.extend(fields.iter().map(|(key, value)| ((*key).to_owned(), value.clone())));
    Json::Object(object).compact()
}

/// `{"type":<kind>,"user":<user>}`, the shape of most account reads.
#[must_use]
pub fn user_request(kind: &str, user: Address) -> String {
    info_request(kind, &[("user", Json::String(user.to_hex()))])
}

/// One read from `/info`, not retried; the caller decides.
pub async fn info<R: DeserializeOwned>(transport: &impl Transport, request: String) -> Result<R, ApiError> {
    let text = transport.post(INFO, request).await?;
    serde_json::from_str(&text).map_err(|error| ApiError::Unreadable { path: INFO, detail: unreadable(&error, &text) })
}

/// Posts a signed body to `/exchange`, once. Never retried, here or by a
/// caller: a second post of a timed-out action could move the money twice.
pub async fn exchange(transport: &impl Transport, body: String) -> Result<Accepted, ApiError> {
    let text = transport.post(EXCHANGE, body).await?;
    match ExchangeResponse::parse(&text)? {
        ExchangeResponse::Ok(accepted) => Ok(accepted),
        ExchangeResponse::Err(reason) => Err(ApiError::Refused(reason)),
    }
}

pub(super) fn unreadable(error: &serde_json::Error, text: &str) -> String {
    let shown: String = text.chars().take(300).collect();
    format!("{error} in {shown}")
}

#[cfg(test)]
#[path = "tests/calls.rs"]
mod tests;
