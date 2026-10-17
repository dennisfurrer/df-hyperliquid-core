//! One network's `/info` and `/exchange` behind one rate-limit gate, the part of a
//! [`crate::client::Client`] that posts. Every read and send passes the gate and the
//! meter; the [`Http`] underneath knows nothing of networks.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::client::Clock;
use crate::client::http::limit::{Gate, Meter, Spent, action_weight, base_weight, per_items, read_weight};
use crate::client::http::transport::{Http, INFO, Request, Transport, TransportError};
use crate::client::network::Network;
use crate::utils::json::Json;

/// How the meter names a post to `/exchange`.
const EXCHANGE_KIND: &str = "exchange";

#[derive(Debug, Default)]
struct Limiter {
    gate: Gate,
    meter: Meter,
}

/// One network's endpoints, rate limited.
pub(crate) struct Api<H> {
    http: H,
    network: Network,
    clock: Clock,
    limiter: Mutex<Limiter>,
}

impl<H: Http> Api<H> {
    pub(crate) fn new(http: H, network: Network, clock: Clock) -> Self {
        Api { http, network, clock, limiter: Mutex::new(Limiter::default()) }
    }

    #[must_use]
    pub(crate) const fn network(&self) -> Network {
        self.network
    }

    #[must_use]
    pub(crate) const fn http(&self) -> &H {
        &self.http
    }

    /// What went out over the last minute, and how long the gate stays closed.
    pub(crate) fn spent(&self) -> Spent {
        let now = (self.clock)();
        let mut limiter = self.limiter();
        let cooldown_ms = limiter.gate.cooldown_ms(now);
        Spent { cooldown_ms, ..limiter.meter.spent(now) }
    }

    fn limiter(&self) -> MutexGuard<'_, Limiter> {
        self.limiter.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<H: Http> Transport for Api<H> {
    /// Past the gate, then to the network's endpoint. A read near the limit
    /// is held back as [`TransportError::RateLimited`] without going out; an
    /// action never is, but a closed gate holds both. Nothing is retried.
    async fn post(&self, path: &str, body: String) -> Result<String, TransportError> {
        let is_read = path == INFO;
        let kind = match is_read {
            true => Json::parse(&body)
                .ok()
                .and_then(|request| request.get("type").and_then(Json::as_str).map(ToOwned::to_owned))
                .unwrap_or_default(),
            false => EXCHANGE_KIND.to_owned(),
        };
        let batched = match is_read {
            true => 0,
            false => batched(&body),
        };
        let ticket = {
            let now = (self.clock)();
            let mut limiter = self.limiter();
            let ticket = limiter.gate.open(now).map_err(|wait| rate_limited(Some(wait)))?;
            if is_read && let Some(wait) = limiter.meter.wait_for(now, base_weight(&kind)) {
                return Err(rate_limited(Some(wait)));
            }
            ticket
        };
        let request = Request::post_json(format!("{}{path}", self.network.api_url()), body);
        let answer = self.http.send(request).await;
        let now = (self.clock)();
        let mut limiter = self.limiter();
        let response = answer?;
        if response.status == 429 {
            limiter.meter.refuse(now);
            let wait = limiter.gate.struck(ticket, now, response.retry_after_ms());
            return Err(rate_limited(Some(wait)));
        }
        let text = response.text();
        let weight = match (is_read, response.is_success()) {
            (true, true) => read_weight(&kind, per_items(&kind).map_or(0, |_| items(&text))),
            (true, false) => base_weight(&kind),
            (false, _) => action_weight(batched),
        };
        limiter.meter.record(now, &kind, weight);
        match response.is_success() {
            true => {
                limiter.gate.passed(ticket);
                Ok(text)
            }
            false => Err(TransportError::Status { status: response.status, body: text }),
        }
    }
}

const fn rate_limited(retry_after_ms: Option<u64>) -> TransportError {
    TransportError::RateLimited { retry_after_ms }
}

/// The orders, cancels or modifies an `/exchange` body batches.
fn batched(body: &str) -> usize {
    let Ok(body) = Json::parse(body) else { return 0 };
    let Some(action) = body.get("action") else { return 0 };
    ["orders", "cancels", "modifies"]
        .iter()
        .find_map(|key| action.get(key).and_then(Json::as_array))
        .map_or(0, <[Json]>::len)
}

/// How many items an answer lists; 0 for anything that is not a list.
fn items(text: &str) -> usize {
    serde_json::from_str::<Vec<serde::de::IgnoredAny>>(text).map_or(0, |list| list.len())
}

#[cfg(test)]
#[path = "tests/api.rs"]
mod tests;
