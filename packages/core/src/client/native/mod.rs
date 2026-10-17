//! HTTP and websocket for native programs: [`Native`] posts through reqwest on
//! rustls, [`clock`] is the system clock, and a [`Feed`] keeps the websocket
//! open over tokio-tungstenite. Hand the first two to a [`crate::Client`]:
//!
//! ```no_run
//! use df_hyperliquid_core::client::native::{Native, clock};
//! use df_hyperliquid_core::{Client, Network};
//!
//! let client = Client::reader(Native::new(), Network::Mainnet, clock);
//! ```
//!
//! A [`Native`] future must be polled inside a tokio runtime. Nothing here
//! retries.

pub mod socket;

use crate::client::http::transport::JSON;
use crate::client::{Http, Request, Response, TransportError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use socket::Feed;

/// How long a request may take, connection and answer together, unless the
/// program says otherwise.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// One HTTP client, cheap to clone: clones share their connections.
#[derive(Debug, Clone)]
pub struct Native {
    client: reqwest::Client,
}

impl Native {
    /// A client with [`TIMEOUT`].
    #[must_use]
    pub fn new() -> Self {
        Native::with_timeout(TIMEOUT)
    }

    /// A client whose every request gives up after `timeout`. A request that
    /// times out comes back as [`TransportError::Unreachable`]: an action may
    /// still have landed.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = reqwest::Client::builder().timeout(timeout).build().unwrap_or_else(|_| reqwest::Client::new());
        Native { client }
    }
}

impl Default for Native {
    fn default() -> Self {
        Native::new()
    }
}

impl Http for Native {
    async fn send(&self, request: Request) -> Result<Response, TransportError> {
        let unreachable = |error: reqwest::Error| TransportError::Unreachable(error.without_url().to_string());
        let mut outgoing = self.client.post(&request.url).header(reqwest::header::CONTENT_TYPE, JSON);
        if !request.body.is_empty() {
            outgoing = outgoing.body(request.body);
        }
        let answer = outgoing.send().await.map_err(unreachable)?;
        let status = answer.status().as_u16();
        let headers = answer
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), String::from_utf8_lossy(value.as_bytes()).into_owned()))
            .collect();
        let body = answer.bytes().await.map_err(unreachable)?.to_vec();
        Ok(Response { status, headers, body })
    }
}

/// Unix milliseconds from the system clock: the [`crate::client::Clock`] for a
/// native program. A clock set before 1970 reads 0.
#[must_use]
pub fn clock() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "tests/http.rs"]
mod tests;
