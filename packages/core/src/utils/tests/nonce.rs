//! Nonces that follow the clock and never repeat.

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
