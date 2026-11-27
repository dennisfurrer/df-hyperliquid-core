//! The rule for REST reads: they wait, orders never do.
//!
//! The venue allows 1,200 units of request weight a minute per IP, reads and actions together,
//! and answers 429 past that. Every `/info` read in this program goes through [`read`], which
//! yields to the quoting loop first, then holds the read while the program's last minute of HTTP
//! weight is over half that limit, and after a 429 holds every read until the cooldown ends. The
//! crate's own gate sits under it: each client holds reads at four fifths of the limit and, after
//! a 429, closes for 2 seconds, doubling to a minute, or for the venue's `Retry-After`.
//!
//! Orders, modifies and cancels never pass through here. Requotes go over the websocket; the
//! switch and the cancels on exit go over HTTP through a client that sends nothing else, so a
//! read's wait or a read's 429 never holds them.

use std::future::Future;
use std::time::Duration;

use df_hyperliquid_core::client::http::limit::LIMIT;
use df_hyperliquid_core::client::{ApiError, Spent, TransportError};

/// Reads wait while the last minute's weight is at or over this: half the venue's limit.
pub const READ_CEILING: u32 = LIMIT / 2;

/// How long a read waits for the gate when no wait was given.
const DEFAULT_WAIT: Duration = Duration::from_secs(2);

/// The longest one read waits in all before it gives up and returns the refusal.
const MOST_WAITING: Duration = Duration::from_secs(30);

/// How long a read waits before it goes out, given what the program's clients spent over the
/// last minute; `None` when it may go now.
pub fn hold(spent: &[Spent]) -> Option<Duration> {
    let cooldown_ms = spent.iter().map(|spent| spent.cooldown_ms).max().unwrap_or(0);
    let minute = spent.iter().fold(0_u32, |sum, spent| sum.saturating_add(spent.minute));
    hold_for(minute, cooldown_ms)
}

/// [`hold`] on its two figures: a closed gate's cooldown, else a second at a time while the
/// minute's weight is at or over [`READ_CEILING`].
pub fn hold_for(minute: u32, cooldown_ms: u64) -> Option<Duration> {
    if cooldown_ms > 0 {
        return Some(Duration::from_millis(cooldown_ms));
    }
    (minute >= READ_CEILING).then_some(Duration::from_secs(1))
}

/// Runs the read `call` under the rule above. `spent` gives what each of the program's clients
/// spent over the last minute; `what` names the read in what is printed.
pub async fn read<T, Fut>(
    what: &str,
    spent: impl Fn() -> Vec<Spent>,
    mut call: impl FnMut() -> Fut,
) -> Result<T, ApiError>
where
    Fut: Future<Output = Result<T, ApiError>>,
{
    let mut waited = Duration::ZERO;
    loop {
        tokio::task::yield_now().await;
        let wait = match hold(&spent()) {
            Some(wait) => wait,
            None => match call().await {
                Err(ApiError::Transport(TransportError::RateLimited { retry_after_ms })) => {
                    retry_after_ms.map_or(DEFAULT_WAIT, Duration::from_millis)
                }
                answer => return answer,
            },
        };
        if waited.saturating_add(wait) > MOST_WAITING {
            return Err(ApiError::Transport(TransportError::RateLimited {
                retry_after_ms: Some(u64::try_from(wait.as_millis()).unwrap_or(u64::MAX)),
            }));
        }
        println!("{what}: read held back {} ms by the rate limit", wait.as_millis());
        tokio::time::sleep(wait).await;
        waited = waited.saturating_add(wait);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_wait_well_before_the_limit_and_after_a_refusal() {
        assert_eq!(hold_for(0, 0), None);
        assert_eq!(hold_for(READ_CEILING - 1, 0), None);
        assert_eq!(READ_CEILING, 600);
        assert_eq!(hold_for(READ_CEILING, 0), Some(Duration::from_secs(1)));
        assert_eq!(hold_for(10, 4_000), Some(Duration::from_millis(4_000)));
    }
}
