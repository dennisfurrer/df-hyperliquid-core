//! The [`Transport`] and [`Http`] traits, the JSON POST that crosses the wire,
//! and the `/info` and `/exchange` calls every read and action goes through.

use std::future::Future;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::utils::address::Address;
use crate::utils::json::Json;

pub const INFO: &str = "/info";
pub const EXCHANGE: &str = "/exchange";

/// Posts a JSON body to `path` (`/info` or `/exchange`) on one network and
/// returns the answer's text. [`crate::client::Client`] implements it behind
/// the rate limit. No `Send` bound, since the browser is single-threaded.
pub trait Transport {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>>;
}

/// One HTTP request out and its answer back, whatever the status. It knows
/// nothing of networks or limits; an error means no answer at all
/// ([`TransportError::Unreachable`]).
pub trait Http {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>>;
}

/// The content type of every request: each is a JSON body, posted.
#[cfg(any(feature = "native", feature = "web"))]
pub(crate) const JSON: &str = "application/json";

/// A JSON body to post to `url`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    pub body: Vec<u8>,
}

impl Request {
    /// A JSON body, posted as `application/json`.
    #[must_use]
    pub fn post_json(url: impl Into<String>, body: String) -> Self {
        Request { url: url.into(), body: body.into_bytes() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// A header's value, its name matched whatever its case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// `Retry-After` in milliseconds, when it is given in whole seconds.
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        let seconds = self.header("retry-after").and_then(|seconds| seconds.trim().parse::<u64>().ok())?;
        Some(seconds.saturating_mul(1_000))
    }

    /// The body as text; bytes that are not UTF-8 read as U+FFFD.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// No answer came back. For `/exchange` that means unknown, not failed:
    /// the action may have landed, so it is never sent again.
    #[error("no answer from the venue: {0}")]
    Unreachable(String),
    #[error("the venue answered HTTP {status}: {body}")]
    Status { status: u16, body: String },
    /// Refused for the rate limit, by the venue (a 429) or by the [`crate::client::Client`]'s
    /// gate before it went out. Nothing was done; try again after the wait.
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
    /// `response.data` whole, as the venue wrote it; `None` when there is none
    /// (`"default"`). Some answers carry more than their statuses.
    pub data: Option<Value>,
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
                Ok(ExchangeResponse::Ok(Accepted { kind, statuses, data: data.cloned() }))
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
#[path = "tests/transport.rs"]
mod tests;
