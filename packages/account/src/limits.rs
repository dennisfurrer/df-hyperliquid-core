//! What an address may still send: `userRateLimit`, and how fast that runs
//! down. Hyperliquid's docs, "Rate limits and user limits" (read 2026-10-08):
//!
//! - every address starts with 10,000 requests and earns one more for every
//!   USDC it trades, cumulatively since it began;
//! - once they are spent it may send one request every 10 seconds;
//! - cancels have `min(limit + 100000, limit * 2)`, so a throttled address
//!   can still cancel;
//! - a batch of `n` orders or cancels is one request to the IP and `n` to
//!   the address;
//! - this limit is on actions only; reads are the IP's ([`hl_core::limit`]);
//! - a subaccount is a user of its own, with its own limit;
//! - an address has 1000 open orders, one more for every 5M USDC of volume,
//!   at most 5000;
//! - `reserveRequestWeight` buys more at 0.0005 USDC a request, paid from the
//!   perps balance: `nRequestsSurplus`.
//!
//! Reading a limit costs the IP 20 and the address nothing, so hip4-tui reads
//! them by hand, at most once a minute ([`READ_EVERY_MS`]) and ten at a time
//! ([`READ_BATCH`]), and measures a pace only between two reads at least a
//! minute apart ([`Budget`]). Nothing here keeps a clock: the time is passed in.

use hl_core::{Address, ApiError, Dec, Round, Transport, info, user_request};
use serde::Deserialize;

/// What every address starts with.
pub const INITIAL_REQUESTS: u64 = 10_000;

/// How often a throttled address may send one request.
pub const THROTTLED_EVERY_MS: u64 = 10_000;

/// The open orders every address has, and the most it can earn.
pub const OPEN_ORDERS: u64 = 1_000;
pub const MOST_OPEN_ORDERS: u64 = 5_000;

/// The volume that earns one more open order, in USDC.
pub const VOLUME_PER_OPEN_ORDER: i64 = 5_000_000;

/// The least time between two reads of the same limits, and the least time
/// a pace is measured over.
pub const READ_EVERY_MS: u64 = 60_000;

/// How many limits hip4-tui reads at once.
pub const READ_BATCH: usize = 10;

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
pub struct RateLimit {
    /// USDC traded since the address began: what buys its cap.
    #[serde(rename = "cumVlm")]
    pub volume: Dec,
    #[serde(rename = "nRequestsUsed")]
    pub used: u64,
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

    #[must_use]
    pub const fn is_throttled(&self) -> bool {
        self.remaining() == 0
    }

    /// What cancels may use.
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

/// What `reserveRequestWeight` charges for `requests` more, in USDC.
#[must_use]
pub fn surplus_cost(requests: u64) -> Option<Dec> {
    Dec::lit::<0>(i128::from(requests)).checked_mul(Dec::lit::<4>(5))
}

/// What an action costs the address: one, or one for every order or cancel
/// it batches.
#[must_use]
pub fn address_requests(batched: usize) -> u64 {
    u64::try_from(batched.max(1)).unwrap_or(u64::MAX)
}

/// The open orders `volume` allows.
#[must_use]
pub fn open_order_cap(volume: Dec) -> u64 {
    let earned = volume
        .checked_div(Dec::from_int(VOLUME_PER_OPEN_ORDER), 0, Round::Down)
        .and_then(|earned| earned.units_at(0, Round::Down))
        .and_then(|earned| u64::try_from(earned).ok())
        .unwrap_or(0);
    OPEN_ORDERS.saturating_add(earned).min(MOST_OPEN_ORDERS)
}

/// How long, from `read_at`, before limits may be read again; `None` when
/// they may be read now. Milliseconds.
#[must_use]
pub fn read_wait_ms(read_at: Option<u64>, now: u64) -> Option<u64> {
    let next = read_at?.saturating_add(READ_EVERY_MS);
    (next > now).then(|| next - now)
}

/// One read of a limit, at `at` milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub at: u64,
    pub limit: RateLimit,
}

/// One address's limit as last read, and the read its pace is measured from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Budget {
    pub last: Option<Sample>,
    /// The latest read at least [`READ_EVERY_MS`] older than `last`.
    pub base: Option<Sample>,
    pub error: Option<String>,
}

impl Budget {
    pub fn took(&mut self, sample: Sample) {
        if let Some(last) = self.last
            && sample.at.saturating_sub(last.at) >= READ_EVERY_MS
        {
            self.base = Some(last);
        }
        self.last = Some(sample);
        self.error = None;
    }

    #[must_use]
    pub fn limit(&self) -> Option<RateLimit> {
        self.last.map(|sample| sample.limit)
    }

    /// Requests spent less those trading earned back, and the milliseconds
    /// that took, between the two reads a pace is measured over.
    fn spent(&self) -> Option<(i128, i128)> {
        let (base, last) = (self.base?, self.last?);
        let ms = i128::from(last.at) - i128::from(base.at);
        if ms <= 0 {
            return None;
        }
        let used = i128::from(last.limit.used) - i128::from(base.limit.used);
        let earned = i128::from(last.limit.cap) - i128::from(base.limit.cap);
        Some((used - earned, ms))
    }

    /// Requests a minute, net of what trading earned back: a bot that trades
    /// as it quotes can be busy and still not run down.
    #[must_use]
    pub fn net_a_minute(&self) -> Option<i64> {
        let (net, ms) = self.spent()?;
        i64::try_from(net * 60_000 / ms).ok()
    }

    /// Milliseconds until nothing is left at the net pace; `None` when it is
    /// not running down, or there is no pace yet.
    #[must_use]
    pub fn runs_out_ms(&self) -> Option<u64> {
        let (net, ms) = self.spent()?;
        if net <= 0 {
            return None;
        }
        let left = i128::from(self.limit()?.remaining());
        u64::try_from(left * ms / net).ok()
    }
}

#[cfg(test)]
#[path = "tests/limits.rs"]
mod tests;
