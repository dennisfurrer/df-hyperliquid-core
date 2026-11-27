//! What keeps resting quotes from outliving the program, and the request budget from running out.
//!
//! - The dead man's switch, `scheduleCancel`: at the time given, the venue cancels every open
//!   order of the account. The exchange docs set two rules: the time is at least 5 seconds ahead,
//!   and it fires at most 10 times a day, counted from 00:00 UTC
//!   (https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/exchange-endpoint, "Schedule
//!   cancel (dead man's switch)"). The venue also refuses it for an account that has not traded
//!   enough: it answered an account with $4,441 traded on 2026-10-31 that $1,000,000 is needed.
//!   The docs do not state that rule, so this program tries the switch once at the start, reads
//!   the answer, and says plainly when it is refused. A refused switch is never sent again in
//!   that run: each refresh is an action that spends the address's request budget, so refreshing
//!   a refused switch every 5 seconds would spend 720 requests an hour for nothing. Only an
//!   accepted switch is refreshed, every [`SWITCH_EVERY`], to [`SWITCH_AHEAD_MS`] ahead; a refusal
//!   of a refresh (for example after the day's 10 triggers) stops the refreshing the same way.
//! - Cancel on exit: on Ctrl-C, on the run time ending, on the loss limit and on any error.
//! - The watchdog: cancels everything when no book update, or no answer to a request, has
//!   arrived for a set time.
//! - The request budget: an address may send 10,000 requests plus one for every USDC it has
//!   traded; each order or cancel in a batch counts. Requotes slow down as the budget runs low.
//!
//! What is left uncovered: a process killed while the network is also down, with the switch
//! refused. Nothing local can cancel then; only the switch, set at the venue, can.

use std::time::{Duration, Instant};

/// How far ahead the switch is armed: 15 seconds, over the venue's 5-second minimum.
pub const SWITCH_AHEAD_MS: u64 = 15_000;

/// How often an accepted switch is armed again, well inside [`SWITCH_AHEAD_MS`].
pub const SWITCH_EVERY: Duration = Duration::from_secs(5);

/// The request budget, read at the start and counted down locally after that.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// What the venue said was left at the start; `None` when it was not read.
    pub at_start: Option<u64>,
    /// Requests sent since the start, by the venue's count: one per order or cancel in a batch.
    pub used: u64,
    /// The shortest gap between requotes while the budget is healthy.
    pub base_gap: Duration,
}

impl Budget {
    /// What is left, as far as this program knows.
    pub fn left(&self) -> Option<u64> {
        self.at_start.map(|start| start.saturating_sub(self.used))
    }

    /// The shortest gap between requotes now: the base gap while plenty is left, then longer
    /// steps, so the budget lasts and the cancels it reserves stay usable.
    pub fn gap(&self) -> Duration {
        match self.left() {
            None => self.base_gap,
            Some(left) if left >= 5_000 => self.base_gap,
            Some(left) if left >= 1_000 => self.base_gap.max(Duration::from_secs(2)),
            Some(left) if left >= 200 => self.base_gap.max(Duration::from_secs(10)),
            Some(_) => self.base_gap.max(Duration::from_secs(60)),
        }
    }
}

/// Watches for silence: no book update, or a request left unanswered, for longer than `limit`.
#[derive(Debug, Clone, Copy)]
pub struct Watchdog {
    /// How long silence is allowed.
    pub limit: Duration,
    /// When the last book update arrived.
    pub last_book: Instant,
}

impl Watchdog {
    /// Why it should cancel everything at `now`, given the oldest unanswered request; `None`
    /// while everything is answering.
    pub fn tripped(&self, now: Instant, oldest_unanswered: Option<Instant>) -> Option<&'static str> {
        if now.saturating_duration_since(self.last_book) > self.limit {
            return Some("no book update");
        }
        match oldest_unanswered {
            Some(sent) if now.saturating_duration_since(sent) > self.limit => Some("a request unanswered"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gap_grows_as_the_budget_runs_down() {
        let mut budget = Budget { at_start: Some(10_000), used: 0, base_gap: Duration::from_millis(500) };
        assert_eq!(budget.gap(), Duration::from_millis(500));
        budget.used = 6_000;
        assert_eq!(budget.gap(), Duration::from_secs(2));
        budget.used = 9_500;
        assert_eq!(budget.gap(), Duration::from_secs(10));
        budget.used = 9_900;
        assert_eq!(budget.gap(), Duration::from_secs(60));
        let unknown = Budget { at_start: None, ..budget };
        assert_eq!(unknown.gap(), Duration::from_millis(500));
    }

    #[test]
    fn the_watchdog_trips_on_silence() {
        let start = Instant::now();
        let dog = Watchdog { limit: Duration::from_secs(10), last_book: start };
        assert_eq!(dog.tripped(start + Duration::from_secs(5), None), None);
        assert_eq!(dog.tripped(start + Duration::from_secs(11), None), Some("no book update"));
        let fresh = Watchdog { last_book: start + Duration::from_secs(20), ..dog };
        assert_eq!(
            fresh.tripped(start + Duration::from_secs(21), Some(start + Duration::from_secs(5))),
            Some("a request unanswered")
        );
    }
}
