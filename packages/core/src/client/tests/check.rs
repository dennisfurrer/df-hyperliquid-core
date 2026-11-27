//! Which signatures the client recovers before posting, in either kind of build.

use super::*;

#[test]
fn a_wallet_is_always_recovered_and_a_local_key_only_in_a_debug_build() {
    for debug_build in [false, true] {
        assert!(recovers(false, debug_build), "a signer that does not vouch, debug {debug_build}");
    }
    assert!(recovers(true, true), "a local key in a debug build");
    assert!(!recovers(true, false), "a local key in a release build");
}
