//! The client: the caller's key, what to ask ([`Call`]), and the one way out, an
//! [`hl_core::Http`]. Every typed read is a [`Call`] built in its own module and sent here.

use std::fmt::{self, Write as _};

use hl_core::{Http, Json, Method, Request, Response, TransportError};
use serde::{Deserialize, de::DeserializeOwned};

/// REST, mainnet: `/info` and `/dashboard/<route>`.
pub const API: &str = "https://api.hydromancer.xyz";

/// The socket, without the key. This is the only form that is ever printed.
pub const SOCKET: &str = "wss://api.hydromancer.xyz/ws";

/// The page that documents the outcome builder dashboard.
pub const DOCS: &str = "https://docs.hydromancer.xyz/readme/outcome-analytics/outcome-builder-dashboard";

/// Upstream text quoted in an error, at most, in characters.
const QUOTED: usize = 300;

/// What the key reads as wherever upstream text would have shown it.
const REDACTED: &str = "<redacted>";

/// The caller's API key. Held here and nowhere else: never printed (its `Debug` is redacted and
/// it has no `Display`), sent as a bearer header, and taken out of any text that quotes the
/// provider.
#[derive(Clone, PartialEq, Eq)]
pub struct Key(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("an API key is one word of visible ASCII")]
pub struct NoKey;

impl Key {
    /// The key as pasted, trimmed. Empty, or holding a space, a control character or anything
    /// outside ASCII, it is no key: it could not go in a header as it is.
    pub fn new(text: &str) -> Result<Key, NoKey> {
        let key = text.trim();
        match !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_graphic()) {
            true => Ok(Key(key.to_owned())),
            false => Err(NoKey),
        }
    }

    /// `text` with the key taken out, in its plain spelling and its URL spelling.
    #[must_use]
    pub fn scrub(&self, text: &str) -> String {
        text.replace(&self.0, REDACTED).replace(&self.escaped(), REDACTED)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }

    /// The key as `encodeURIComponent` writes it, which is how a browser's socket URL carries it.
    fn escaped(&self) -> String {
        escape(&self.0)
    }
}

/// Written out rather than derived, so the key is redacted instead of printed.
impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(<redacted>)")
    }
}

/// `encodeURIComponent`: everything but letters, digits and `-_.!~*'()` as `%XX`.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            true => out.push(char::from(byte)),
            false => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// One request to Hydromancer, without the key: [`Hydromancer::request`] adds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    method: Method,
    /// The path and its query, after [`API`].
    path: String,
    body: String,
    /// The route or the request's `type`, which an error names.
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadCall {
    #[error("`{0}` is not a dashboard route: letters, digits, `-` and `_` only")]
    Route(String),
    #[error("`{0}` cannot go in a URL as it is: no spaces, control characters or `#`")]
    Query(String),
}

impl Call {
    /// `GET /dashboard/<route>?<query>`, the query as typed. A route is one path segment, so a
    /// call never leaves the dashboard; a query keeps to what a URL carries as written.
    pub fn dashboard(route: &str, query: &str) -> Result<Call, BadCall> {
        let plain =
            !route.is_empty() && route.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte));
        if !plain {
            return Err(BadCall::Route(route.to_owned()));
        }
        if !query.bytes().all(|byte| byte.is_ascii_graphic() && byte != b'#') {
            return Err(BadCall::Query(query.to_owned()));
        }
        let path = match query.is_empty() {
            true => format!("/dashboard/{route}"),
            false => format!("/dashboard/{route}?{query}"),
        };
        Ok(Call { method: Method::Get, path, body: String::new(), name: route.to_owned() })
    }

    /// `POST /info` with a JSON body, named by its `type`.
    #[must_use]
    pub fn info(body: &Json) -> Call {
        let name = body.get("type").and_then(Json::as_str).unwrap_or("info").to_owned();
        Call { method: Method::Post, path: "/info".to_owned(), body: body.compact(), name }
    }

    /// A dashboard route with its parameters, each value escaped so none can add another.
    pub(crate) fn route(route: &'static str, query: &[(&str, &str)]) -> Call {
        let query: Vec<String> = query.iter().map(|(name, value)| format!("{name}={}", escape(value))).collect();
        Call {
            method: Method::Get,
            path: format!("/dashboard/{route}?{}", query.join("&")),
            body: String::new(),
            name: route.to_owned(),
        }
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("{API}{}", self.path)
    }

    #[must_use]
    pub const fn method(&self) -> Method {
        self.method
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Why a read failed. Every text in it was scrubbed of the key before it was put here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No answer came back.
    #[error("no answer: {0}")]
    Unreachable(String),
    /// An answer other than a 2xx, with the provider's own words where it gave them.
    #[error("{name} answered HTTP {status}: {said}")]
    Status { name: String, status: u16, said: String },
    #[error("the answer to {name} could not be read: {detail}")]
    Unreadable { name: String, detail: String },
}

impl Error {
    /// Whether the same request might succeed later: no answer, a rate limit or a 5xx. Anything
    /// else is the provider's answer about the request itself, and a retry gets it again.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Error::Unreachable(_) => true,
            Error::Status { status, .. } => *status == 429 || *status / 100 == 5,
            Error::Unreadable { .. } => false,
        }
    }

    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Error::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// Hydromancer over an [`Http`], with the caller's key.
pub struct Hydromancer<H> {
    http: H,
    key: Key,
}

/// Written out rather than derived, so the key is redacted instead of printed.
impl<H> fmt::Debug for Hydromancer<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hydromancer").field("api", &API).field("key", &self.key).finish()
    }
}

impl<H> Hydromancer<H> {
    /// Hydromancer serves mainnet; there is no other constructor, so a testnet session has no
    /// client to ask.
    pub const fn mainnet(http: H, key: Key) -> Self {
        Hydromancer { http, key }
    }

    #[must_use]
    pub const fn http(&self) -> &H {
        &self.http
    }

    /// The request for `call`, the key in its `Authorization` header.
    #[must_use]
    pub fn request(&self, call: &Call) -> Request {
        let request = match call.method {
            Method::Get => Request::get(call.url()),
            Method::Post => Request::post_json(call.url(), call.body.clone()),
        };
        request.with_header("Authorization", self.key.bearer())
    }

    /// `text` with the key taken out.
    #[must_use]
    pub fn scrub(&self, text: &str) -> String {
        self.key.scrub(text)
    }

    /// The socket's URL with the key, which a browser's socket can carry nowhere else. It holds
    /// the key: open the socket with it and never print it.
    #[must_use]
    pub fn socket_url(&self) -> String {
        format!("{SOCKET}?token={}", self.key.escaped())
    }

    pub(crate) fn unreadable(&self, call: &Call, detail: &str) -> Error {
        Error::Unreadable { name: call.name.clone(), detail: self.scrub(detail) }
    }
}

impl<H: Http> Hydromancer<H> {
    /// Sends `call` once and gives back a 2xx answer as it came. Nothing is retried here; the
    /// caller decides ([`Error::is_retryable`]).
    pub async fn send(&self, call: &Call) -> Result<Response, Error> {
        let response = self.http.send(self.request(call)).await.map_err(|error| self.lost(&error))?;
        match response.is_success() {
            true => Ok(response),
            false => {
                Err(Error::Status { name: call.name.clone(), status: response.status, said: self.said(&response.body) })
            }
        }
    }

    /// Sends `call` and reads its JSON answer as `T`.
    pub async fn read<T: DeserializeOwned>(&self, call: &Call) -> Result<T, Error> {
        let response = self.send(call).await?;
        serde_json::from_slice(&response.body).map_err(|error| self.unreadable(call, &error.to_string()))
    }

    /// The escape hatch: any call, its JSON answer as written, every number its digits.
    pub async fn raw(&self, call: &Call) -> Result<Json, Error> {
        let response = self.send(call).await?;
        Json::parse(&response.text()).map_err(|error| self.unreadable(call, &error.to_string()))
    }

    fn lost(&self, error: &TransportError) -> Error {
        let said = match error {
            TransportError::Unreachable(said) => said.clone(),
            other => other.to_string(),
        };
        Error::Unreachable(self.scrub(&said))
    }

    /// The provider's refusal in its own words: `{"error": "..."}`, else the body, cut short.
    fn said(&self, body: &[u8]) -> String {
        #[derive(Deserialize)]
        struct Said {
            error: String,
        }
        let said = match serde_json::from_slice::<Said>(body) {
            Ok(said) => said.error,
            Err(_) => String::from_utf8_lossy(body).into_owned(),
        };
        self.scrub(&said).chars().take(QUOTED).collect()
    }
}

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;
