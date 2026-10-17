//! Plans against the recorded testnet stake in `testnet.json` (validator
//! `bob node`'s own, 2026-10-08).

use crate::client::Network;
use crate::utils::sign::UserAction;
use crate::utils::{Address, BadUnits, Dec};
use serde::Deserialize;

use super::*;
use crate::earn::staking::reads::{Delegation, DelegatorSummary};

const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/testnet.json"));
const NONCE: u64 = 1_790_412_105_001;

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

#[test]
fn every_staking_action_needs_the_account_s_own_key() {
    for kind in ["cDeposit", "cWithdraw", "tokenDelegate"] {
        assert_eq!(key(kind), Some(Key::Main), "{kind}");
    }
    assert_eq!(key("vaultTransfer"), None);
    assert!(!Key::Main.takes_agent() && Key::Either.takes_agent());
    assert_eq!([CDeposit::KEY, CWithdraw::KEY, TokenDelegate::KEY], [Key::Main; 3]);
}

/// Every move is read back on the staking balance, which nothing but these
/// moves changes: rewards go to the delegations.
#[test]
fn each_move_is_read_back_on_the_staking_balance_by_exactly_its_amount() {
    let ctx = Ctx { network: Network::Mainnet, nonce: NONCE, now_ms: NONCE };
    let enough = summary("200", 0);
    let into = ctx.stake(dec("100")).unwrap();
    assert_eq!(into.effect, Effect { balance: Balance::Undelegated, change: Change::Up(dec("100")) });
    assert_eq!(into.free_at, None);
    let out = ctx.unstake(dec("1.5"), &enough).unwrap();
    assert_eq!(out.effect, Effect { balance: Balance::Undelegated, change: Change::Down(dec("1.5")) });
    assert_eq!(out.free_at, Some(NONCE + 7 * 86_400_000), "in spot after the queue");
    let delegate = ctx.delegate(bob(), dec("100"), &enough).unwrap();
    assert_eq!(delegate.effect, Effect { balance: Balance::Undelegated, change: Change::Down(dec("100")) });
    assert_eq!(delegate.free_at, Some(NONCE + 86_400_000), "locked a day");
    assert!(delegate.effect.landed(dec("200"), dec("100")));
    assert!(!delegate.effect.landed(dec("200"), dec("200")), "not landed yet");
    assert_eq!((delegate.network, delegate.nonce, delegate.key), (Network::Mainnet, NONCE, Key::Main));
}

#[test]
fn testnet_frees_staked_hype_by_its_own_clock() {
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: NONCE };
    assert_eq!(ctx.unstake(dec("1"), &summary("1", 0)).unwrap().free_at, Some(NONCE + 300_000));
    assert_eq!(ctx.delegate(bob(), dec("1"), &summary("1", 0)).unwrap().free_at, Some(NONCE + 120_000));
}

#[test]
fn what_the_staking_balance_does_not_hold_is_refused() {
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: NONCE };
    let recorded = recorded();
    assert_eq!(recorded.delegator_summary.undelegated, Dec::ZERO);
    assert_eq!(
        ctx.unstake(dec("1"), &recorded.delegator_summary),
        Err(Refused::MoreThan { amount: Dec::ONE, held: Dec::ZERO })
    );
    assert_eq!(
        ctx.delegate(bob(), dec("1"), &recorded.delegator_summary),
        Err(Refused::MoreThan { amount: Dec::ONE, held: Dec::ZERO })
    );
    assert_eq!(ctx.unstake(dec("1"), &summary("10", 5)), Err(Refused::QueueFull), "five may wait at once");
    assert!(ctx.unstake(dec("1"), &summary("10", 4)).is_ok());
    assert_eq!(
        ctx.stake(dec("0.000000001")),
        Err(Refused::Units(BadUnits::TooFine { value: dec("0.000000001"), places: 8 }))
    );
    assert_eq!(ctx.stake(Dec::ZERO), Err(Refused::Units(BadUnits::NotPositive(Dec::ZERO))));
}

/// The recorded delegation was locked until 1778137877166.
#[test]
fn an_undelegation_waits_for_the_lock_and_takes_at_most_what_is_delegated() {
    let delegations = recorded().delegations;
    let until = 1_778_137_877_166;
    let early = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: until - 1 };
    assert_eq!(early.undelegate(bob(), dec("1"), &delegations), Err(Refused::Locked { until }));
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: until };
    let plan = ctx.undelegate(bob(), dec("968879.07903711"), &delegations).unwrap();
    assert!(plan.action.wire(Network::Testnet).contains(r#""wei":96887907903711,"isUndelegate":true"#));
    assert_eq!(plan.effect, Effect { balance: Balance::Undelegated, change: Change::Up(dec("968879.07903711")) });
    assert_eq!(
        ctx.undelegate(bob(), dec("968879.07903712"), &delegations),
        Err(Refused::MoreThan { amount: dec("968879.07903712"), held: dec("968879.07903711") })
    );
    let other: Address = "0x172054cfc01b32effe0bf6af7a15b36e1ad730b3".parse().unwrap();
    assert_eq!(ctx.undelegate(other, dec("1"), &delegations), Err(Refused::NoDelegation(other)));
}
