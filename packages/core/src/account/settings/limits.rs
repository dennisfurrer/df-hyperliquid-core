//! What an address may still send (`userRateLimit`), the venue's formulas for what it earns:
//! requests by volume traded, cancels past the cap, open orders by volume; and
//! `reserveRequestWeight`, which buys more requests outright. Action limits are per
//! address, a subaccount having its own; reads are the IP's ([`crate::client::http::limit`]).

use crate::account::{ActionKind, Signers};
use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{Address, Dec, Round};
use serde::Deserialize;

/// What every address starts with; it earns one more request for every USDC it trades.
pub const INITIAL_REQUESTS: u64 = 10_000;

/// How often a throttled address may send one request.
pub const THROTTLED_EVERY_MS: u64 = 10_000;

/// The open orders every address has.
pub const OPEN_ORDERS: u64 = 1_000;
/// The most open orders an address can earn.
pub const MOST_OPEN_ORDERS: u64 = 5_000;

/// The volume that earns one more open order, in USDC.
pub const VOLUME_PER_OPEN_ORDER: i64 = 5_000_000;

/// What one reserved request costs, in USDC, paid from the perps balance.
pub const RESERVED_REQUEST_COST: Dec = Dec::lit::<4>(5);

/// The most weight one `reserveRequestWeight` reserves, as `@nktkas/hyperliquid` 0.33.3 bounds it.
pub const MOST_RESERVED_WEIGHT: u64 = 1_844_674_407_370_955;

/// A reservation the venue would not take.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a reservation is 1 to {MOST_RESERVED_WEIGHT} requests, not {0}")]
pub struct BadWeight(pub u64);

/// `reserveRequestWeight`: `{type, weight, destination}`, the destination only when set. Buys
/// `weight` more requests at [`RESERVED_REQUEST_COST`] each, for the signer's account or for
/// another account that already exists. An agent may sign it for its master.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveRequestWeight {
    weight: u64,
    destination: Option<Address>,
}

impl ReserveRequestWeight {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::ReserveRequestWeight.signers();

    /// `weight` more requests for the account that signs.
    pub fn new(weight: u64) -> Result<ReserveRequestWeight, BadWeight> {
        match (1..=MOST_RESERVED_WEIGHT).contains(&weight) {
            true => Ok(ReserveRequestWeight { weight, destination: None }),
            false => Err(BadWeight(weight)),
        }
    }

    /// For `destination` instead, paid by the account that signs.
    #[must_use]
    pub const fn for_user(mut self, destination: Address) -> ReserveRequestWeight {
        self.destination = Some(destination);
        self
    }

    /// How many requests it reserves.
    #[must_use]
    pub const fn weight(&self) -> u64 {
        self.weight
    }

    /// The account the requests are for; `None` for the one that signs.
    #[must_use]
    pub const fn destination(&self) -> Option<Address> {
        self.destination
    }

    /// What it costs, in USDC.
    #[must_use]
    pub fn cost(&self) -> Option<Dec> {
        Dec::new(i128::from(self.weight), 0)?.checked_mul(RESERVED_REQUEST_COST)
    }
}

impl L1Action for ReserveRequestWeight {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "reserveRequestWeight")
            .put("weight", self.weight)
            .put_some("destination", self.destination.map(|destination| destination.to_hex()))
    }
}

/// `{"type":"userRateLimit","user":..}`.
#[must_use]
pub fn rate_limit_request(user: Address) -> String {
    user_request("userRateLimit", user)
}

/// One address's limit. Weighs 20 against the IP.
pub async fn rate_limit(transport: &impl Transport, user: Address) -> Result<RateLimit, ApiError> {
    info(transport, rate_limit_request(user)).await
}

/// `userRateLimit`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct RateLimit {
    /// USDC traded since the address began: what buys its cap.
    #[serde(rename = "cumVlm")]
    pub volume: Dec,
    /// Requests sent so far (`nRequestsUsed`), throttled ones included.
    #[serde(rename = "nRequestsUsed")]
    pub used: u64,
    /// The requests the address may send before it is throttled (`nRequestsCap`).
    #[serde(rename = "nRequestsCap")]
    pub cap: u64,
    /// Bought with `reserveRequestWeight`.
    #[serde(rename = "nRequestsSurplus", default)]
    pub surplus: u64,
}

impl RateLimit {
    /// What is left before the address is throttled; never a wrapped total,
    /// since throttled requests still count.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.cap.saturating_sub(self.used)
    }

    /// Whether the address has used its cap, so it may send one request every
    /// [`THROTTLED_EVERY_MS`].
    #[must_use]
    pub const fn is_throttled(&self) -> bool {
        self.remaining() == 0
    }

    /// What cancels may use: `min(cap + 100000, cap * 2)`, so a throttled address can still cancel.
    #[must_use]
    pub const fn cancel_cap(&self) -> u64 {
        let wider = self.cap.saturating_add(100_000);
        let doubled = self.cap.saturating_mul(2);
        if wider < doubled { wider } else { doubled }
    }

    /// The cap `volume` earns: 10,000 and one for every whole USDC. `None`
    /// for a negative volume.
    #[must_use]
    pub fn earned_cap(volume: Dec) -> Option<u64> {
        let whole = u64::try_from(volume.units_at(0, Round::Down)?).ok()?;
        INITIAL_REQUESTS.checked_add(whole)
    }
}

/// The USDC to trade to earn `requests` more.
#[must_use]
pub fn volume_for(requests: u64) -> Dec {
    Dec::lit::<0>(i128::from(requests))
}

/// What `reserveRequestWeight` charges for `requests` more, in USDC: 0.0005 a request, paid from
/// the perps balance.
#[must_use]
pub fn surplus_cost(requests: u64) -> Option<Dec> {
    Dec::lit::<0>(i128::from(requests)).checked_mul(RESERVED_REQUEST_COST)
}

/// What an action costs the address: one, or one for every order or cancel it batches. The IP
/// counts a batch as one.
#[must_use]
pub fn address_requests(batched: usize) -> u64 {
    u64::try_from(batched.max(1)).unwrap_or(u64::MAX)
}

/// The open orders `volume` allows: one more for every 5M USDC, at most 5000.
#[must_use]
pub fn open_order_cap(volume: Dec) -> u64 {
    let earned = volume
        .checked_div(Dec::from_int(VOLUME_PER_OPEN_ORDER), 0, Round::Down)
        .and_then(|earned| earned.units_at(0, Round::Down))
        .and_then(|earned| u64::try_from(earned).ok())
        .unwrap_or(0);
    OPEN_ORDERS.saturating_add(earned).min(MOST_OPEN_ORDERS)
}

#[cfg(test)]
#[path = "tests/limits.rs"]
mod tests;
