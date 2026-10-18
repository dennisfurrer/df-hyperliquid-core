//! The [`Http`] wire: one request out, with its method, headers and body, and
//! its answer back whatever the status. A host or a dependent plugs in here.

use std::future::Future;

/// One HTTP request out and its answer back, whatever the status. It knows
/// nothing of networks or limits; an error means no answer at all
/// ([`TransportError::Unreachable`]).
pub trait Http {
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    #[must_use]
    pub fn get(url: impl Into<String>) -> Self {
        Request { method: Method::Get, url: url.into(), headers: Vec::new(), body: Vec::new() }
    }

    /// A JSON body, posted as `application/json`.
    #[must_use]
    pub fn post_json(url: impl Into<String>, body: String) -> Self {
        Request {
            method: Method::Post,
            url: url.into(),
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: body.into_bytes(),
        }
    }

    /// The same request with one more header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// A header's value, its name matched whatever its case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        find_header(&self.headers, name)
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
        find_header(&self.headers, name)
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

fn find_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
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

#[cfg(test)]
#[path = "tests/wire.rs"]
mod tests;
