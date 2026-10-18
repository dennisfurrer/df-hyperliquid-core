//! The official Python SDK's `signing_test.py` order vectors.

use crate::client::Network;
use crate::trade::orders::*;
use crate::utils::sign::L1Action;

use super::{asset, dec, key, limit};

/// `test_l1_action_signing_order_matches` and
/// `test_l1_action_signing_tpsl_order_matches`: ETH, asset 1, 100 at 100,
/// nonce 0. Python drops a leading zero from `r` and `s`; these have none.
#[test]
fn the_python_sdk_s_order_vectors_sign_the_same() {
    let key = key();
    let plain = PlaceOrder::one(limit(1, Side::Buy, "100", "100", Tif::Gtc));
    let stop =
        PlaceOrder::one(Order::trigger(asset(1), Side::Buy, dec("100"), dec("100"), dec("103"), Tpsl::StopLoss, true));
    for (action, r, s, v) in [
        (
            plain,
            "0xd65369825a9df5d80099e513cce430311d7d26ddf477f5b3a33d2806b100d78e",
            "0x2b54116ff64054968aa237c20ca9ff68000f977c93289157748a3162b6ea940e",
            28,
        ),
        (
            stop,
            "0x98343f2b5ae8e26bb2587daad3863bc70d8792b09af1841b6fdd530a2065a3f9",
            "0x6b5bb6bb0633b710aa22b721dd9dee6d083646a5f8e581a20b545be6c1feb405",
            27,
        ),
    ] {
        let signature = key.sign_digest(&action.digest(0, None, Network::Mainnet)).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{action:?}");
    }
}
