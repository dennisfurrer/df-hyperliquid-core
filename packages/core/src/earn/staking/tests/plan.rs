//! Plans against the recorded testnet stake in `testnet.json` (validator `bob node`'s own,
//! 2026-10-08): every move read back on the staking balance and freed by its network's clock, and
//! everything the staking balance does not hold, or holds locked, refused before signing.

use crate::client::Network;
use crate::utils::sign::UserAction;
use crate::utils::{Address, BadUnits, Dec};
use serde::Deserialize;

use super::*;
use crate::earn::staking::actions::hype_wei;
use crate::earn::staking::reads::{Delegation, DelegatorSummary};

const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/testnet.json"));
const NONCE: u64 = 1_790_412_105_001;
const DAY: u64 = 86_400_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recorded {
    delegator_summary: DelegatorSummary,
    delegations: Vec<Delegation>,
}

fn recorded() -> Recorded {
    serde_json::from_str(TESTNET).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn bob() -> Address {
    "0x0594f60525fc6a5d05bb711fb6450f84fef307bd".parse().unwrap()
}

fn summary(undelegated: &str, pending: u64) -> DelegatorSummary {
    DelegatorSummary {
        delegated: Dec::ZERO,
        undelegated: dec(undelegated),
        total_pending_withdrawal: Dec::ZERO,
        n_pending_withdrawals: pending,
    }
}

fn at(network: Network, now_ms: u64) -> Ctx {
    Ctx { network, nonce: NONCE, now_ms }
}

/// Every move passes through the staking balance, which nothing else changes (rewards go to the
/// delegations), so each is read back there by exactly its amount. A withdrawal is free in spot
/// once its queue ends, a delegation once its lock does; each needs the account's own key.
#[test]
fn each_move_is_read_back_on_the_staking_balance_and_freed_by_its_network_s_clock() {
    assert_eq!([CDeposit::KEY, CWithdraw::KEY, TokenDelegate::KEY], [Signers::Main; 3]);
    let enough = summary("200", 0);
    let undelegated = |change| Effect { balance: Balance::Undelegated, change };
    for (network, queue, lock) in [(Network::Mainnet, 7 * DAY, DAY), (Network::Testnet, 300_000, 120_000)] {
        let ctx = at(network, NONCE);
        let into = ctx.stake(dec("100")).unwrap();
        assert_eq!((into.effect, into.free_at), (undelegated(Change::Up(dec("100"))), None));
        let out = ctx.unstake(dec("1.5"), &enough).unwrap();
        assert_eq!((out.effect, out.free_at), (undelegated(Change::Down(dec("1.5"))), Some(NONCE + queue)));
        let delegate = ctx.delegate(bob(), dec("100"), &enough).unwrap();
        assert_eq!((delegate.effect, delegate.free_at), (undelegated(Change::Down(dec("100"))), Some(NONCE + lock)));
        assert_eq!((delegate.network, delegate.nonce, delegate.key), (network, NONCE, Signers::Main));
    }
    let until = 1_778_137_877_166;
    let back = at(Network::Testnet, until).undelegate(bob(), dec("968879.07903711"), &recorded().delegations).unwrap();
    assert!(back.action.wire(Network::Testnet).contains(r#""wei":96887907903711,"isUndelegate":true"#));
    assert_eq!((back.effect, back.free_at), (undelegated(Change::Up(dec("968879.07903711"))), None));
    let down = undelegated(Change::Down(dec("100")));
    assert!(down.landed(dec("200"), dec("100")) && !down.landed(dec("200"), dec("200")), "not landed yet");
    assert!(undelegated(Change::Up(dec("1"))).landed(dec("1"), dec("2")));
}

/// More than the staking balance holds, a sixth withdrawal while five wait, a delegation before
/// its lock ends (the recorded one was locked until 1778137877166) or to a validator not delegated
/// to, and HYPE finer than eight places or not above zero.
#[test]
fn what_the_staking_balance_does_not_hold_or_holds_locked_is_refused() {
    let recorded = recorded();
    let ctx = at(Network::Testnet, NONCE);
    let until = 1_778_137_877_166;
    let early = at(Network::Testnet, until - 1);
    let other: Address = "0x172054cfc01b32effe0bf6af7a15b36e1ad730b3".parse().unwrap();
    let nothing = Refused::MoreThan { amount: Dec::ONE, held: Dec::ZERO };
    let nine_places = dec("0.000000001");
    let cases: Vec<(Result<(), Refused>, Refused)> = vec![
        (ctx.unstake(Dec::ONE, &recorded.delegator_summary).map(drop), nothing.clone()),
        (ctx.delegate(bob(), Dec::ONE, &recorded.delegator_summary).map(drop), nothing),
        (ctx.unstake(Dec::ONE, &summary("10", 5)).map(drop), Refused::QueueFull),
        (early.undelegate(bob(), Dec::ONE, &recorded.delegations).map(drop), Refused::Locked { until }),
        (
            at(Network::Testnet, until).undelegate(bob(), dec("968879.07903712"), &recorded.delegations).map(drop),
            Refused::MoreThan { amount: dec("968879.07903712"), held: dec("968879.07903711") },
        ),
        (ctx.undelegate(other, Dec::ONE, &recorded.delegations).map(drop), Refused::NoDelegation(other)),
        (ctx.stake(nine_places).map(drop), Refused::Units(BadUnits::TooFine { value: nine_places, places: 8 })),
        (ctx.stake(Dec::ZERO).map(drop), Refused::Units(BadUnits::NotPositive(Dec::ZERO))),
    ];
    for (planned, refused) in cases {
        assert_eq!(planned, Err(refused));
    }
    assert!(ctx.unstake(Dec::ONE, &summary("10", 4)).is_ok(), "five may wait at once");
    assert_eq!(hype_wei(dec("1.5")), Ok(150_000_000));
}
