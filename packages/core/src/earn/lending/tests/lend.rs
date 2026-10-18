//! `borrowLend`'s amount and a user's lending as the venue writes it. The `borrowLend` vectors are
//! in `tests/conformance/lend.rs`; the reserves in `tests/math.rs`; the reads through a client in
//! `tests/mocked/earn.rs`.

use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::*;
use crate::earn::lending::action::{BadLend, BorrowLend, Operation};

const SUPPLIER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/lend/user_state_mainnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn read(text: &str) -> UserState {
    serde_json::from_str(text).unwrap()
}

/// `null` is all of it, which only a withdraw or a repay takes; any amount given is above zero.
#[test]
fn null_means_all_and_only_withdraw_and_repay_take_it() {
    assert_eq!(BorrowLend::withdraw_all(0).amount(), None);
    assert!(BorrowLend::withdraw_all(0).wire().ends_with(r#""operation":"withdraw","token":0,"amount":null}"#));
    assert_eq!(BorrowLend::repay_all(150).operation(), Operation::Repay);
    assert_eq!(BorrowLend::borrow(150, Dec::ZERO), Err(BadLend::NotPositive(Dec::ZERO)), "zero is not all");
    assert_eq!(BorrowLend::withdraw(0, dec("-1")), Err(BadLend::NotPositive(dec("-1"))));
    assert_eq!(
        BorrowLend::repay(0, dec("1.50")).unwrap().wire(),
        r#"{"type":"borrowLend","operation":"repay","token":0,"amount":"1.5"}"#
    );
    let names: Vec<&str> = Operation::ALL.iter().map(|operation| operation.wire()).collect();
    assert_eq!(names, ["supply", "withdraw", "borrow", "repay"]);
}

/// An HLP strategy supplying USDC (`user_state_mainnet.json`, live 2026-10-18); live answers of
/// 2026-10-06 for an account that once touched two reserves, both empty now, and one that never
/// touched any; and a health factor, which no live account showed, read as written either way.
#[test]
fn a_user_s_positions_and_health_read_as_written() {
    let supplier = read(SUPPLIER);
    let usdc = supplier.position(0).unwrap();
    assert_eq!((usdc.supply.basis, usdc.supply.value), (dec("100011000"), dec("100542851.6265490204")));
    assert_eq!(usdc.supply.interest(), Some(dec("531851.6265490204")));
    assert_eq!(
        (supplier.supplied(0), supplier.borrowed(0), supplier.supplied(150)),
        (usdc.supply.value, Dec::ZERO, Dec::ZERO)
    );
    assert!(supplier.is_healthy() && supplier.factor().is_none());

    let touched = read(concat!(
        r#"{"tokenToState":[[0,{"borrow":{"basis":"0.0","value":"0.0"},"supply":{"basis":"0.0","value":"0.0"}}],"#,
        r#"[360,{"borrow":{"basis":"0.0","value":"0.0"},"supply":{"basis":"0.0","value":"0.0"}}]],"#,
        r#""health":"healthy","healthFactor":null}"#
    ));
    assert_eq!(touched.token_to_state.len(), 2, "zero rows stay listed");
    assert_eq!(touched.positions().count(), 0, "and hold nothing");
    assert_eq!(read(r#"{"tokenToState":[],"health":"healthy","healthFactor":null}"#).position(0), None);
    for said in [r#""1.8732""#, "1.8732"] {
        let state = read(&format!(r#"{{"tokenToState":[],"health":"healthy","healthFactor":{said}}}"#));
        assert_eq!(state.factor(), Some(dec("1.8732")), "{said}");
    }
}
