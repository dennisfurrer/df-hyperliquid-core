//! The `borrowLend` vectors were made by `@nktkas/hyperliquid` 0.33.2 itself,
//! as hip4-tui recorded them: its own action function, the key below, nonce
//! 1790412105001, a transport that kept the body. Connection ids are the
//! SDK's `createL1ActionHash`. Mainnet.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{L1Action, LocalKey, TransportError, hex};

use super::*;
use crate::action::{BadLend, BorrowLend, Operation};

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_790_412_105_001;

/// Live, mainnet, 2026-10-06: the whole answer.
const RESERVES: &str = r#"[[0,{"borrowYearlyRate":"0.05","supplyYearlyRate":"0.0344823466","balance":"126365878.8608557433","utilization":"0.766274368","oraclePx":"1.0","ltv":"0.0","totalSupplied":"540658780.1005587578","totalBorrowed":"414292965.0398716927"}],[150,{"borrowYearlyRate":"0.05","supplyYearlyRate":"0.0","balance":"13401729.9812650606","utilization":"0.0","oraclePx":"91.865","ltv":"0.65","totalSupplied":"13401729.9812650606","totalBorrowed":"0.0"}],[197,{"borrowYearlyRate":"0.05","supplyYearlyRate":"0.0","balance":"2221.393184613","utilization":"0.0","oraclePx":"85661.713","ltv":"0.5","totalSupplied":"2221.393184613","totalBorrowed":"0.0"}],[268,{"borrowYearlyRate":"0.05","supplyYearlyRate":"0.0230191762","balance":"1351671.6160367201","utilization":"0.5115372486","oraclePx":"0.99974","ltv":"0.0","totalSupplied":"2767194.6979851001","totalBorrowed":"1415523.1622486101"}],[360,{"borrowYearlyRate":"0.05","supplyYearlyRate":"0.0049329595","balance":"153754.84479192","utilization":"0.1096213233","oraclePx":"1.0","ltv":"0.0","totalSupplied":"172684.38565121","totalBorrowed":"18929.890866"}]]"#;
/// Live, the same day: an account that once touched two reserves, both empty
/// now, and one that never touched any.
const TOUCHED: &str = r#"{"tokenToState":[[0,{"borrow":{"basis":"0.0","value":"0.0"},"supply":{"basis":"0.0","value":"0.0"}}],[360,{"borrow":{"basis":"0.0","value":"0.0"},"supply":{"basis":"0.0","value":"0.0"}}]],"health":"healthy","healthFactor":null}"#;
const FRESH: &str = r#"{"tokenToState":[],"health":"healthy","healthFactor":null}"#;
/// hip4-tui's recorded answer for a supplier.
const SUPPLIER: &str = r#"{"tokenToState":[[0,{"borrow":{"basis":"0.0","value":"0.0"},"supply":{"basis":"99.1","value":"100.25"}}]],"health":"healthy","healthFactor":null}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn borrow_lend_is_the_sdk_s_byte_for_byte() {
    let key = LocalKey::from_hex(KEY).unwrap();
    let cases = [
        (
            BorrowLend::supply(0, dec("100.50")).unwrap(),
            r#"{"type":"borrowLend","operation":"supply","token":0,"amount":"100.5"}"#,
            "0x1d8d7b38e50758c4781c5cc17e2bb64dcd31c7e1870293da248ebb3ad9743730",
            "0x165ca9fe188b157c2203f6d8e1ebd2204d7b939652d29c0626d0c4ca642e662f",
            "0x1e22b483499c2d5f0ccaf2773bfd22cde99cb372f3f437bbf125d636e294edfd",
            28,
        ),
        (
            BorrowLend::repay_all(150),
            r#"{"type":"borrowLend","operation":"repay","token":150,"amount":null}"#,
            "0xa43161edaad13340450e7eb286453bbe1aca5ec7e278baefb171120a4a7e26f4",
            "0xf0e249f385d377f2748b79f88ac039feebd4c458012bacd8e09839ea5d725585",
            "0x7fec0e9e5ab28c4f79e28c2b2c27fd8d3f02db82de777fa1cd6bf1019b45830b",
            27,
        ),
    ];
    for (action, json, id, r, s, v) in cases {
        assert_eq!(action.wire(), json);
        assert_eq!(hex::encode_prefixed(&action.connection_id(NONCE, None)), id, "{json}");
        let signature = key.sign_digest(&action.digest(NONCE, None)).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(
            action.body(NONCE, &signature, None),
            format!(
                r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}},"vaultAddress":null}}"#
            )
        );
    }
}

#[test]
fn null_means_all_and_only_withdraw_and_repay_take_it() {
    assert_eq!(BorrowLend::withdraw_all(0).amount(), None);
    assert!(BorrowLend::withdraw_all(0).wire().ends_with(r#""operation":"withdraw","token":0,"amount":null}"#));
    assert_eq!(BorrowLend::borrow(150, Dec::ZERO), Err(BadLend::NotPositive(Dec::ZERO)), "zero is not all");
    assert_eq!(BorrowLend::withdraw(0, dec("-1")), Err(BadLend::NotPositive(dec("-1"))));
    let names: Vec<&str> = Operation::ALL.iter().map(|operation| operation.wire()).collect();
    assert_eq!(names, ["supply", "withdraw", "borrow", "repay"]);
}

#[test]
fn every_reserve_reads_from_the_live_answer() {
    let reserves: Vec<(u32, ReserveState)> = serde_json::from_str(RESERVES).unwrap();
    assert_eq!(reserves.iter().map(|(token, _)| *token).collect::<Vec<_>>(), [0, 150, 197, 268, 360]);
    let (_, usdc) = &reserves[0];
    assert_eq!((usdc.borrow_yearly_rate, usdc.supply_yearly_rate), (dec("0.05"), dec("0.0344823466")));
    assert_eq!((usdc.utilization, usdc.ltv, usdc.oracle_px), (dec("0.766274368"), Dec::ZERO, Dec::ONE));
    assert_eq!(usdc.balance, dec("126365878.8608557433"));
    assert_ne!(
        usdc.total_supplied.checked_sub(usdc.total_borrowed),
        Some(usdc.balance),
        "live, the balance is not exactly supplied less borrowed"
    );
    let (_, hype) = &reserves[1];
    assert_eq!((hype.ltv, hype.oracle_px), (dec("0.65"), dec("91.865")));
}

#[test]
fn a_user_s_positions_and_health_read() {
    let touched: UserState = serde_json::from_str(TOUCHED).unwrap();
    assert_eq!(touched.token_to_state.len(), 2);
    assert_eq!(touched.position(360).map(|position| position.supply.value), Some(Dec::ZERO));
    assert!(touched.is_healthy() && touched.health_factor.is_none());
    let fresh: UserState = serde_json::from_str(FRESH).unwrap();
    assert_eq!(fresh.position(0), None);
    let supplier: UserState = serde_json::from_str(SUPPLIER).unwrap();
    let usdc = supplier.position(0).unwrap();
    assert_eq!((usdc.supply.basis, usdc.supply.value, usdc.borrow.value), (dec("99.1"), dec("100.25"), Dec::ZERO));
}

/// No live account showed a health factor; whether it comes as a string or
/// a bare number, it reads as written.
#[test]
fn a_health_factor_reads_as_written_either_way() {
    for said in [r#""1.8732""#, "1.8732"] {
        let state: UserState =
            serde_json::from_str(&format!(r#"{{"tokenToState":[],"health":"healthy","healthFactor":{said}}}"#))
                .unwrap();
        assert_eq!(state.health_factor.and_then(|factor| factor.to_dec()), Some(dec("1.8732")), "{said}");
    }
}

struct Recorded {
    sent: RefCell<Vec<String>>,
}

impl Transport for Recorded {
    fn post(&self, _: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push(body);
        ready(Ok(FRESH.to_owned()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn the_reads_name_their_type() {
    let transport = Recorded { sent: RefCell::new(Vec::new()) };
    let user: Address = "0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap();
    assert!(block_on(user_state(&transport, user)).unwrap().is_healthy());
    assert_eq!(
        transport.sent.borrow()[0],
        r#"{"type":"borrowLendUserState","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414"}"#
    );
    assert_eq!(all_reserve_states_request(), r#"{"type":"allBorrowLendReserveStates"}"#);
}
