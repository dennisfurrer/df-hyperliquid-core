//! [`NonceClock`], the source of action nonces.

/// One account's nonces: milliseconds, strictly increasing. The venue keeps
/// the highest nonces it has seen per signer and refuses a repeat, so each
/// account holds its own clock. The time is passed in, since the browser and
/// Workers have no std clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NonceClock {
    last: u64,
}

impl NonceClock {
    /// A clock that has handed out nothing yet.
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

    /// The last nonce handed out, or the `last` it was made with; 0 for a new clock.
    #[must_use]
    pub const fn last(&self) -> u64 {
        self.last
    }
}

#[cfg(test)]
#[path = "tests/nonce.rs"]
mod tests;
