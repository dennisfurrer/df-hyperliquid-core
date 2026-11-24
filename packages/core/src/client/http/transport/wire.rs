//! The [`Http`] wire: one request out, with its method, headers and body, and
//! its answer back whatever the status. A host or a dependent plugs in here.

use std::future::Future;

/// One HTTP request out and its answer back, whatever the status. It knows
/// nothing of networks or limits; an error means no answer at all
/// ([`TransportError::Unreachable`]).
pub trait Http {
    /// Sends `request` and returns the answer, whatever its status.
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, TransportError>>;
}

/// An HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
}

/// An HTTP request for an [`Http`] to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `GET` or `POST`.
    pub method: Method,
    /// The full URL, such as `https://api.hyperliquid.xyz/info`.
    pub url: String,
    /// Header names and values, in the order they are sent.
    pub headers: Vec<(String, String)>,
    /// The body bytes; empty for a `GET`.
    pub body: Vec<u8>,
}

impl Request {
    /// A `GET` of `url` with no headers and no body.
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

    /// Whether it carries a signature: a post to `/exchange`, or a raw HyperEVM transaction.
    #[must_use]
    pub fn is_signed(&self) -> bool {
        const RAW: &[u8] = b"eth_sendRawTransaction";
        self.method == Method::Post
            && (self.url.trim_end_matches('/').ends_with("/exchange")
                || self.body.windows(RAW.len()).any(|window| window == RAW))
    }

    /// [`TransportError::Offline`] for a signed request in an `offline` build; otherwise nothing.
    pub fn refuse_if_offline(&self) -> Result<(), TransportError> {
        match OFFLINE && self.is_signed() {
            true => Err(TransportError::Offline(self.url.clone())),
            false => Ok(()),
        }
    }
}

/// The answer to a [`Request`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// Header names and values, as received.
    pub headers: Vec<(String, String)>,
    /// The body bytes.
    pub body: Vec<u8>,
}

impl Response {
    /// Whether the status is 2xx.
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

/// Why a request got no usable answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// No answer came back. For `/exchange` that means unknown, not failed:
    /// the action may have landed, so it is never sent again.
    #[error("no answer from the venue: {0}")]
    Unreachable(String),
    /// The venue answered with a status other than 2xx or 429.
    #[error("the venue answered HTTP {status}: {body}")]
    Status {
        /// The HTTP status code.
        status: u16,
        /// The answer's text.
        body: String,
    },
    /// Refused for the rate limit, by the venue (a 429) or by the [`crate::client::Client`]'s
    /// gate before it went out. Nothing was done; try again after the wait.
    #[error("rate limited; try again in {} ms", retry_after_ms.unwrap_or(0))]
    RateLimited {
        /// How long to wait, in milliseconds: the venue's `Retry-After` or the
        /// gate's cooldown. `None` when no wait was given.
        retry_after_ms: Option<u64>,
    },
    /// Not sent: this build has the `offline` feature, and the request carried a signature.
    #[error("an offline build sends nothing signed: {0}")]
    Offline(String),
}

/// Whether this build's transports refuse what is signed ([`TransportError::Offline`]).
pub const OFFLINE: bool = cfg!(feature = "offline");

#[cfg(test)]
#[path = "tests/wire.rs"]
mod tests;
