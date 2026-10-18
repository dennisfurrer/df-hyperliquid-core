//! Nonces that follow the clock and never repeat, one count an account.

use super::*;

#[test]
fn a_nonce_follows_the_clock_never_repeats_and_is_kept_an_account_each() {
    let mut clock = NonceClock::new();
    for (now, nonce) in [(10, 10), (10, 11), (10, 12), (5, 13), (100, 100)] {
        assert_eq!(clock.next(now), nonce, "at {now}: a clock that steps back does not take the nonce with it");
    }
    assert_eq!(clock.last(), 100);

    let mut one = NonceClock::new();
    let mut two = NonceClock::after(1_790_000_000_000);
    assert_eq!(one.next(1_790_000_000_000), 1_790_000_000_000);
    assert_eq!(two.next(1_790_000_000_000), 1_790_000_000_001);
    assert_eq!(one.last(), 1_790_000_000_000);
}
