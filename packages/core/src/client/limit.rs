//! Request weights against Hyperliquid's rate limit (the docs' "Rate limits
//! and user limits"), the last minute's spend, and the gate a 429 closes. No
//! clock and no I/O: every call is handed the time.

use std::collections::VecDeque;

/// Weight a minute per IP, reads and actions together.
pub const LIMIT: u32 = 1_200;

/// What reads may spend of [`LIMIT`] a minute before they wait. The last fifth
/// is left to actions, which never wait, and to other clients on the same IP.
pub const SOFT: u32 = LIMIT / 5 * 4;

/// The window every figure here is over.
pub const WINDOW_MS: u64 = 60_000;

/// The first cooldown a 429 closes the gate for; each strike doubles it.
pub const BASE_COOLDOWN_MS: u64 = 2_000;

/// The longest cooldown a strike sets, though a `Retry-After` may ask longer.
pub const MAX_COOLDOWN_MS: u64 = 60_000;

const MAX_STRIKES: u32 = 6;

/// An `/info` read's weight before counting what it returns.
#[must_use]
pub fn base_weight(kind: &str) -> u32 {
    match kind {
        "l2Book" | "allMids" | "clearinghouseState" | "orderStatus" | "spotClearinghouseState" | "exchangeStatus" => 2,
        "userRole" => 60,
        _ => 20,
    }
}

/// For the reads that charge for what they return: how many items add one
/// unit of weight.
#[must_use]
pub fn per_items(kind: &str) -> Option<usize> {
    match kind {
        "recentTrades"
        | "historicalOrders"
        | "userFills"
        | "userFillsByTime"
        | "fundingHistory"
        | "userFunding"
        | "nonUserFundingUpdates"
        | "twapHistory"
        | "userTwapSliceFills"
        | "userTwapSliceFillsByTime"
        | "delegatorHistory"
        | "delegatorRewards"
        | "validatorStats" => Some(20),
        "candleSnapshot" => Some(60),
        _ => None,
    }
}

/// A read of `kind` that returned `items`.
#[must_use]
pub fn read_weight(kind: &str, items: usize) -> u32 {
    let extra = per_items(kind).map_or(0, |per| items / per);
    base_weight(kind).saturating_add(u32::try_from(extra).unwrap_or(u32::MAX))
}

/// An action batching `batched` orders, cancels or modifies: 1, plus 1 per 40.
#[must_use]
pub fn action_weight(batched: usize) -> u32 {
    1u32.saturating_add(u32::try_from(batched / 40).unwrap_or(u32::MAX))
}

/// What went out over the last minute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spent {
    pub limit: u32,
    pub minute: u32,
    /// By kind, the heaviest first.
    pub kinds: Vec<Kind>,
    /// 429s over the last minute.
    pub refused: u32,
    /// How long the gate stays closed; 0 when it is open.
    pub cooldown_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kind {
    pub name: String,
    pub count: u32,
    pub weight: u32,
}

/// The last minute of requests: when each went out, its kind and weight; and
/// when each refusal came.
#[derive(Debug, Default)]
pub struct Meter {
    sent: VecDeque<(u64, String, u32)>,
    refused: VecDeque<u64>,
}

impl Meter {
    /// Drops what is a whole window old at `now`.
    fn prune(&mut self, now: u64) {
        let gone = |at: u64| at.saturating_add(WINDOW_MS) <= now;
        while self.sent.front().is_some_and(|(at, _, _)| gone(*at)) {
            self.sent.pop_front();
        }
        while self.refused.front().is_some_and(|at| gone(*at)) {
            self.refused.pop_front();
        }
    }

    pub fn record(&mut self, now: u64, kind: &str, weight: u32) {
        self.prune(now);
        self.sent.push_back((now, kind.to_owned(), weight));
    }

    pub fn refuse(&mut self, now: u64) {
        self.prune(now);
        self.refused.push_back(now);
    }

    /// How long a read of `need` waits at `now`, so the last minute stays
    /// within [`SOFT`]: none while it fits, else until enough has rolled off.
    pub fn wait_for(&mut self, now: u64, need: u32) -> Option<u64> {
        self.prune(now);
        let spent = self.sent.iter().fold(0u32, |sum, (_, _, weight)| sum.saturating_add(*weight));
        let mut over = spent.saturating_add(need).checked_sub(SOFT).filter(|over| *over > 0)?;
        for (at, _, weight) in &self.sent {
            if *weight >= over {
                return Some(at.saturating_add(WINDOW_MS).saturating_sub(now).max(1));
            }
            over -= weight;
        }
        Some(WINDOW_MS)
    }

    /// The last minute at `now`, with no cooldown: the [`Gate`] knows that.
    pub fn spent(&mut self, now: u64) -> Spent {
        self.prune(now);
        let mut kinds: Vec<Kind> = Vec::new();
        for (_, name, weight) in &self.sent {
            match kinds.iter_mut().find(|kind| kind.name == *name) {
                Some(kind) => {
                    kind.count += 1;
                    kind.weight = kind.weight.saturating_add(*weight);
                }
                None => kinds.push(Kind { name: name.clone(), count: 1, weight: *weight }),
            }
        }
        kinds.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.name.cmp(&b.name)));
        let minute = kinds.iter().fold(0u32, |sum, kind| sum.saturating_add(kind.weight));
        Spent {
            limit: LIMIT,
            minute,
            kinds,
            refused: u32::try_from(self.refused.len()).unwrap_or(u32::MAX),
            cooldown_ms: 0,
        }
    }
}

/// What a request carries out through the [`Gate`]: how many strikes there
/// had been when it went out, so a burst sent before a 429 counts once.
#[derive(Debug, Clone, Copy)]
pub struct Ticket(u64);

/// Closed by a 429 for every request, reads and actions alike: the venue
/// limits by IP, so one refusal means the next request would be refused too.
#[derive(Debug, Default)]
pub struct Gate {
    until: u64,
    /// Consecutive strikes, a burst counting once.
    strikes: u32,
    /// Strikes ever.
    epoch: u64,
}

impl Gate {
    /// A ticket out, or how long the gate stays closed.
    pub fn open(&self, now: u64) -> Result<Ticket, u64> {
        match self.until > now {
            true => Err(self.until - now),
            false => Ok(Ticket(self.epoch)),
        }
    }

    /// The request was answered: room again, unless a 429 struck since it
    /// went out.
    pub fn passed(&mut self, ticket: Ticket) {
        if ticket.0 == self.epoch {
            self.strikes = 0;
        }
    }

    /// A 429 came back at `now`. The first of a burst strikes: two seconds,
    /// doubling to a minute, or a longer `Retry-After`. The rest of the burst
    /// only moves the gate for a longer `Retry-After`. Gives the wait.
    pub fn struck(&mut self, ticket: Ticket, now: u64, retry_after_ms: Option<u64>) -> u64 {
        let wait = match ticket.0 == self.epoch {
            true => {
                self.epoch += 1;
                self.strikes = (self.strikes + 1).min(MAX_STRIKES);
                let backoff = (BASE_COOLDOWN_MS << (self.strikes - 1)).min(MAX_COOLDOWN_MS);
                backoff.max(retry_after_ms.unwrap_or(0))
            }
            false => retry_after_ms.unwrap_or(0),
        };
        self.until = self.until.max(now.saturating_add(wait));
        self.until.saturating_sub(now)
    }

    #[must_use]
    pub const fn cooldown_ms(&self, now: u64) -> u64 {
        self.until.saturating_sub(now)
    }
}

#[cfg(test)]
#[path = "tests/limit.rs"]
mod tests;
