//! The info and exchange client, over whatever posts JSON: `fetch` in the
//! browser and in Workers, an HTTP client natively, a recording in tests.

use std::future::Future;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{address::Address, json::Json};

pub const INFO: &str = "/info";
pub const EXCHANGE: &str = "/exchange";

/// Posts a JSON body to `path` (`/info` or `/exchange`) on one network and
/// gives back the answer's text. [`crate::Api`] is the one to use: it names
/// the network and keeps the rate limit. A test may answer from a recording.
/// No `Send` bound: the browser is single-threaded.
pub trait Transport {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>>;
}

/// The wire: posts a JSON body to a whole URL and gives back the answer's
/// text. It knows nothing of networks or limits; `fetch` in the browser and
/// in Workers, an HTTP client natively. A 429 comes back as
/// [`TransportError::RateLimited`] with its `Retry-After`, or as a plain
/// [`TransportError::Status`].
pub trait Http {
    fn post(&self, url: &str, body: String) -> impl Future<Output = Result<String, TransportError>>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// No answer came back. For `/exchange` that means unknown, not failed:
    /// the action may have landed, so it is never sent again.
    #[error("no answer from the venue: {0}")]
    Unreachable(String),
    #[error("the venue answered HTTP {status}: {body}")]
    Status { status: u16, body: String },
    /// Refused for the rate limit, by the venue (a 429) or by the gate
    /// before it went out. Nothing was done; try again after the wait.
    #[error("rate limited; try again in {} ms", retry_after_ms.unwrap_or(0))]
    RateLimited { retry_after_ms: Option<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("the venue's answer on {path} could not be read: {detail}")]
    Unreadable { path: &'static str, detail: String },
    /// `{"status":"err"}`: the venue's own words, shown as they came.
    #[error("{0}")]
    Refused(String),
}

/// `{"type":<kind>,"user":<user>}`, the shape of most account reads.
#[must_use]
pub fn user_request(kind: &str, user: Address) -> String {
    Json::Object(vec![
        ("type".to_owned(), Json::String(kind.to_owned())),
        ("user".to_owned(), Json::String(user.to_hex())),
    ])
    .compact()
}

/// One read from `/info`. Reads may be retried; this does not, the caller
/// decides.
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

fn unreadable(error: &serde_json::Error, text: &str) -> String {
    let shown: String = text.chars().take(300).collect();
    format!("{error} in {shown}")
}

/// What `/exchange` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeResponse {
    /// `{"status":"ok","response":{...}}`.
    Ok(Accepted),
    /// `{"status":"err","response":"message"}`.
    Err(String),
}

/// An accepted action. `kind` is `response.type`: `"default"` for a transfer,
/// `"order"` and the like for actions that answer per item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    pub kind: String,
    /// `response.data.statuses`, or `response.data.status` alone, when present.
    pub statuses: Vec<Status>,
}

/// One item of an accepted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok(Value),
    /// `{"error": "..."}`: that item was refused though the action was taken.
    Error(String),
}

impl Accepted {
    /// The items the venue refused inside an `ok`.
    pub fn errors(&self) -> impl Iterator<Item = &str> {
        self.statuses.iter().filter_map(|status| match status {
            Status::Error(reason) => Some(reason.as_str()),
            Status::Ok(_) => None,
        })
    }
}

impl ExchangeResponse {
    pub fn parse(text: &str) -> Result<ExchangeResponse, ApiError> {
        let bad = |detail: String| ApiError::Unreadable { path: EXCHANGE, detail };
        let answer: Value = serde_json::from_str(text).map_err(|error| bad(unreadable(&error, text)))?;
        let response = answer.get("response").cloned().unwrap_or(Value::Null);
        match answer.get("status").and_then(Value::as_str) {
            Some("ok") => {
                let kind = response.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
                let data = response.get("data");
                let statuses = match (
                    data.and_then(|data| data.get("statuses")).and_then(Value::as_array),
                    data.and_then(|data| data.get("status")),
                ) {
                    (Some(many), _) => many.iter().map(status_of).collect(),
                    (None, Some(one)) => vec![status_of(one)],
                    (None, None) => Vec::new(),
                };
                Ok(ExchangeResponse::Ok(Accepted { kind, statuses }))
            }
            Some(_) => Ok(ExchangeResponse::Err(text_of(&response))),
            None => Err(bad(format!("no status in {}", text.chars().take(300).collect::<String>()))),
        }
    }
}

fn status_of(status: &Value) -> Status {
    match status.get("error") {
        Some(error) => Status::Error(text_of(error)),
        None => Status::Ok(status.clone()),
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().map_or_else(|| value.to_string(), ToOwned::to_owned)
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
