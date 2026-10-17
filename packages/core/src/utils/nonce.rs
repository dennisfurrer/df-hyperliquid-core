/// One account's nonces: milliseconds, strictly increasing. The venue keeps
/// the highest nonces it has seen per signer and refuses a repeat, so each
/// account holds its own clock. The time is passed in, since the browser and
/// Workers have no std clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NonceClock {
    last: u64,
}

impl NonceClock {
    #[must_use]
    pub const fn new() -> Self {
        NonceClock { last: 0 }
    }

    /// A clock that hands out nothing at or below `last`.
    #[must_use]
    pub const fn after(last: u64) -> Self {
        NonceClock { last }
    }

    /// `max(now_ms, last + 1)`, remembered as the new last.
    pub fn next(&mut self, now_ms: u64) -> u64 {
        let nonce = now_ms.max(self.last.saturating_add(1));
        self.last = nonce;
        nonce
    }

    #[must_use]
    pub const fn last(&self) -> u64 {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonce_follows_the_clock_and_never_repeats() {
        let mut clock = NonceClock::new();
        assert_eq!(clock.next(10), 10);
        assert_eq!(clock.next(10), 11);
        assert_eq!(clock.next(10), 12);
        assert_eq!(clock.next(5), 13, "a clock that steps back does not take the nonce with it");
        assert_eq!(clock.next(100), 100);
        assert_eq!(clock.last(), 100);
    }

    #[test]
    fn two_accounts_keep_their_own_counts() {
        let mut one = NonceClock::new();
        let mut two = NonceClock::after(1_790_000_000_000);
        assert_eq!(one.next(1_790_000_000_000), 1_790_000_000_000);
        assert_eq!(two.next(1_790_000_000_000), 1_790_000_000_001);
        assert_eq!(one.last(), 1_790_000_000_000);
    }
}
