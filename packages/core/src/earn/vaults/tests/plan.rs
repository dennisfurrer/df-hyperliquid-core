//! Plans against the recorded vaults in `details.json` and `equities.json`
//! (2026-10-08).

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::sign::L1Action;
use crate::utils::{BadUnits, Dec};

use super::*;
use crate::earn::vaults::reads::{VaultDetails, VaultEquity};
use crate::earn::vaults::transfer::{TESTNET_HLP, VaultTransfer};

const DETAILS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/details.json"));
const EQUITIES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/equities.json"));
const NONCE: u64 = 1_790_412_105_001;
const DAY: u64 = 86_400_000;

fn details(name: &str) -> VaultDetails {
    let all: BTreeMap<String, VaultDetails> = serde_json::from_str(DETAILS).unwrap();
    all[name].clone()
}

/// The testnet follower's HLP share: 1068.95511, locked until 1765926140668.
fn equity() -> VaultEquity {
    let all: BTreeMap<String, Vec<VaultEquity>> = serde_json::from_str(EQUITIES).unwrap();
    all["testnetHlpFollower"][0].clone()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn an_agent_may_move_into_and_out_of_a_vault() {
    assert_eq!(key("vaultTransfer"), Some(Key::Either));
    assert_eq!(key("borrowLend"), None);
    assert_eq!(VaultTransfer::KEY, Key::Either);
}

/// A vault's equity moves with its profit and loss too, so a read back shows
/// the direction; the deposit is locked for the vault's lockup.
#[test]
fn a_deposit_rises_the_equity_and_locks_for_the_vault_s_lockup() {
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: NONCE };
    let punks = details("testnetHyperpunks");
    let plan = ctx.deposit(&punks, dec("10")).unwrap();
    assert_eq!(plan.effect, Effect { balance: Balance::Vault(punks.vault_address), change: Change::Rises(dec("10")) });
    assert_eq!((plan.key, plan.free_at), (Key::Either, Some(NONCE + 20_000)));
    assert!(plan.action.wire().contains(r#""isDeposit":true,"usd":10000000"#));
    let hlp = ctx.deposit(&details("testnetHlp"), dec("10")).unwrap();
    assert_eq!(hlp.free_at, Some(NONCE + 4 * DAY));
    assert!(hlp.effect.landed(dec("1068.9"), dec("1078.95")));
    assert!(!hlp.effect.landed(dec("1068.9"), dec("1068.9")));
    let mainnet = Ctx { network: Network::Mainnet, nonce: NONCE, now_ms: NONCE };
    assert_eq!(mainnet.deposit(&details("mainnetGrowi"), dec("10")).unwrap().free_at, Some(NONCE + DAY));
}

#[test]
fn a_vault_that_takes_no_deposits_is_refused_one() {
    let ctx = Ctx { network: Network::Mainnet, nonce: NONCE, now_ms: NONCE };
    let mut closed = details("mainnetGrowi");
    closed.is_closed = true;
    assert_eq!(ctx.deposit(&closed, dec("10")), Err(Refused::NoDeposits(closed.vault_address)));
    let mut shut = details("mainnetGrowi");
    shut.allow_deposits = false;
    assert_eq!(ctx.deposit(&shut, dec("10")), Err(Refused::NoDeposits(shut.vault_address)));
    assert_eq!(
        ctx.deposit(&details("mainnetGrowi"), dec("1.0000001")),
        Err(Refused::Units(BadUnits::TooFine { value: dec("1.0000001"), places: 6 }))
    );
}

#[test]
fn a_withdrawal_waits_for_the_lockup_and_takes_at_most_the_equity() {
    let equity = equity();
    let until = 1_765_926_140_668;
    let early = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: until - 1 };
    assert_eq!(early.withdraw(&equity, dec("1")), Err(Refused::Locked { until }));
    assert_eq!(early.withdraw_all(&equity), Err(Refused::Locked { until }));
    let ctx = Ctx { network: Network::Testnet, nonce: NONCE, now_ms: until };
    let some = ctx.withdraw(&equity, dec("100")).unwrap();
    assert_eq!(some.effect, Effect { balance: Balance::Vault(TESTNET_HLP), change: Change::Falls(dec("100")) });
    assert_eq!((some.key, some.free_at), (Key::Either, None));
    assert_eq!(
        ctx.withdraw(&equity, dec("1068.955111")),
        Err(Refused::MoreThan { amount: dec("1068.955111"), held: dec("1068.95511") })
    );
    let all = ctx.withdraw_all(&equity).unwrap();
    assert_eq!(all.action.usd(), 1_068_955_110);
    assert!(
        all.action.wire().contains(r#""vaultAddress":"0xa15099a30bbf2e68942d6f4c43d70d04faeab0a0","isDeposit":false"#)
    );
    assert_eq!(all.effect.change, Change::Falls(dec("1068.95511")));
}
