//! Plans against the recorded reserves (`reserves.json`, both networks,
//! 2026-10-08) and a portfolio-margin account recorded on mainnet, 2026-10-02.

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::*;
use crate::earn::lending::action::{BadLend, BorrowLend};
use crate::earn::lending::state::{ReserveState, UserState};

const RESERVES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/lend/reserves.json"));
const ATLAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/lend/atlas_pm.json"));
const NONCE: u64 = 1_790_412_105_001;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn reserves(network: Network) -> Vec<(u32, ReserveState)> {
    let all: BTreeMap<String, Vec<(u32, ReserveState)>> = serde_json::from_str(RESERVES).unwrap();
    all[network.name()].clone()
}

/// Supplied 3.32730523 HYPE (150), borrowed 188.39764406 USDC (0).
fn atlas() -> UserState {
    let all: serde_json::Value = serde_json::from_str(ATLAS).unwrap();
    serde_json::from_value(all["borrowLendUserState"].clone()).unwrap()
}

#[test]
fn an_agent_may_lend_and_borrow() {
    assert_eq!(key("borrowLend"), Some(Key::Either));
    assert_eq!(key("usdSend"), None);
    assert_eq!(BorrowLend::KEY, Key::Either);
    assert!(Key::Either.takes_agent() && !Key::Main.takes_agent());
}

/// A position's value moves with interest too, so a read back shows which
/// way it went, or that it emptied.
#[test]
fn each_operation_is_read_back_on_its_own_position() {
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE };
    let testnet = reserves(Network::Testnet);
    let supply = ctx.supply(1105, dec("2"), &testnet).unwrap();
    assert_eq!(supply.effect, Effect { balance: Balance::Supplied(1105), change: Change::Rises(dec("2")) });
    assert_eq!((supply.network, supply.nonce, supply.key), (Network::Testnet, NONCE, Key::Either));
    assert!(supply.action.wire().contains(r#""operation":"supply","token":1105,"amount":"2""#));
    let borrow = ctx.borrow(0, dec("50"), &testnet).unwrap();
    assert_eq!(borrow.effect, Effect { balance: Balance::Borrowed(0), change: Change::Rises(dec("50")) });
    assert!(supply.effect.landed(dec("1"), dec("3.00000001")));
    assert!(!supply.effect.landed(dec("1"), dec("1")));

    let ctx = Ctx { network: Network::Mainnet, nonce: NONCE };
    let state = atlas();
    let some = ctx.withdraw(150, Some(dec("1")), &state).unwrap();
    assert_eq!(some.effect, Effect { balance: Balance::Supplied(150), change: Change::Falls(dec("1")) });
    let all = ctx.withdraw(150, None, &state).unwrap();
    assert_eq!(all.action, BorrowLend::withdraw_all(150));
    assert_eq!(all.effect, Effect { balance: Balance::Supplied(150), change: Change::Empties });
    assert!(all.effect.landed(dec("3.32730523"), Dec::ZERO) && !all.effect.landed(dec("3.3"), dec("0.1")));
    let repay = ctx.repay(0, None, &state).unwrap();
    assert_eq!(repay.action, BorrowLend::repay_all(0));
    assert_eq!(repay.effect, Effect { balance: Balance::Borrowed(0), change: Change::Empties });
    let part = ctx.repay(0, Some(dec("88.39764406")), &state).unwrap();
    assert!(part.effect.landed(dec("188.39764406"), dec("100.0001")));
}

#[test]
fn what_is_not_there_is_refused() {
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE };
    let testnet = reserves(Network::Testnet);
    assert_eq!(ctx.supply(150, dec("1"), &testnet), Err(Refused::NoReserve(150)), "testnet's HYPE is 1105");
    assert_eq!(
        ctx.borrow(0, dec("2868585.28603627"), &testnet),
        Err(Refused::MoreThan { amount: dec("2868585.28603627"), held: dec("2868585.28603626") }),
        "more than the reserve holds"
    );
    assert_eq!(ctx.supply(0, Dec::ZERO, &testnet), Err(Refused::Lend(BadLend::NotPositive(Dec::ZERO))));

    let ctx = Ctx { network: Network::Mainnet, nonce: NONCE };
    let state = atlas();
    assert_eq!(
        ctx.withdraw(150, Some(dec("4")), &state),
        Err(Refused::MoreThan { amount: dec("4"), held: dec("3.32730523") })
    );
    assert_eq!(ctx.withdraw(0, None, &state), Err(Refused::Nothing(0)), "nothing supplied");
    assert_eq!(ctx.repay(150, None, &state), Err(Refused::Nothing(150)), "nothing owed");
    assert_eq!(
        ctx.repay(0, Some(dec("200")), &state),
        Err(Refused::MoreThan { amount: dec("200"), held: dec("188.39764406") })
    );
}
