//! `details.json` and `equities.json` are live answers read on 2026-10-08,
//! as the venue wrote them, cut to a few followers and one portfolio row:
//!
//! - `vaultDetails` of mainnet's HLP, one of its strategies, and a user vault
//!   (Growi HF); testnet's HLP, asked about one follower, and a user vault
//!   (Hyperpunks). The followers kept are each one deposit in, so their
//!   lockup is the vault's.
//! - `userVaultEquities` of that testnet follower, and of another in two
//!   vaults.

use std::collections::BTreeMap;

use crate::client::Network;
use crate::utils::{Address, Dec, Figure};

use super::*;
use crate::earn::vaults::transfer::{MAINNET_HLP, TESTNET_HLP, hlp, lockup_ms};

const DETAILS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/details.json"));
const EQUITIES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/equities.json"));
const DAY: u64 = 86_400_000;

fn details() -> BTreeMap<String, VaultDetails> {
    serde_json::from_str(DETAILS).unwrap()
}

fn equities() -> BTreeMap<String, Vec<VaultEquity>> {
    serde_json::from_str(EQUITIES).unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// HLP locks four days on both networks; any other vault a day on mainnet
/// and twenty seconds on testnet.
#[test]
fn each_network_s_lockups_are_what_its_followers_show() {
    let all = details();
    for (name, network, lockup) in [
        ("mainnetHlp", Network::Mainnet, 4 * DAY),
        ("mainnetGrowi", Network::Mainnet, DAY),
        ("testnetHlp", Network::Testnet, 4 * DAY),
        ("testnetHyperpunks", Network::Testnet, 20_000),
    ] {
        let vault = &all[name];
        assert_eq!(lockup_ms(network, vault.vault_address), lockup, "{name}");
        assert!(!vault.followers.is_empty(), "{name}");
        for follower in &vault.followers {
            let (entry, until) = (follower.vault_entry_time.unwrap(), follower.lockup_until.unwrap());
            assert_eq!(until - entry, lockup, "{name}: {:?}", follower.user);
        }
    }
    assert_eq!((hlp(Network::Mainnet), hlp(Network::Testnet)), (MAINNET_HLP, TESTNET_HLP));
    assert_eq!(all["mainnetHlp"].vault_address, MAINNET_HLP);
    assert_eq!(all["testnetHlp"].vault_address, TESTNET_HLP);
    assert_eq!(lockup_ms(Network::Testnet, MAINNET_HLP), 20_000, "mainnet's HLP is no HLP on testnet");
}

#[test]
fn a_vault_says_whether_it_leads_strategies_or_is_one() {
    let all = details();
    let child = address("0x010461c14e146ac35fe42271bdc1134ee31c703a");
    match &all["mainnetHlp"].relationship {
        Relationship::Parent { child_addresses } => {
            assert_eq!(child_addresses.len(), 7);
            assert!(child_addresses.contains(&child));
        }
        other => panic!("HLP is a parent, not {other:?}"),
    }
    assert_eq!(all["mainnetHlpChild"].vault_address, child);
    assert_eq!(all["mainnetHlpChild"].relationship, Relationship::Child);
    assert_eq!(all["mainnetHlpChild"].leader, MAINNET_HLP, "a strategy is led by HLP");
    assert_eq!(all["mainnetGrowi"].relationship, Relationship::Normal);
    assert_eq!(all["testnetHyperpunks"].relationship, Relationship::Normal);
}

/// Floats as the venue wrote them, an exponent included.
#[test]
fn every_float_keeps_its_digits() {
    let all = details();
    let testnet = &all["testnetHlp"];
    assert_eq!(testnet.leader_fraction.as_ref().map(Figure::as_str), Some("6.644969985732533e-7"));
    assert_eq!(testnet.leader_fraction.as_ref().and_then(Figure::to_dec), Dec::new(664_496_998_573, 18));
    let child = &all["mainnetHlpChild"];
    assert_eq!(child.leader_fraction.as_ref().map(Figure::as_str), Some("1.0000000000003149"));
    assert_eq!(all["mainnetGrowi"].leader_commission.as_ref().and_then(Figure::to_dec), Some(dec("0.1")));
}

/// What may leave now: the asking follower's `maxWithdrawable` once its
/// lockup has passed, which for this one is its equity cut to millionths.
#[test]
fn what_may_leave_a_vault_now() {
    let all = details();
    let testnet = &all["testnetHlp"];
    let mine = testnet.follower_state.as_ref().unwrap();
    assert_eq!(mine.vault_equity, dec("1068.9562528158"));
    let until = mine.lockup_until.unwrap();
    assert_eq!(testnet.withdrawable_now(until), dec("1068.956252"));
    assert_eq!(testnet.withdrawable_now(until - 1), Dec::ZERO, "locked");
    assert!(mine.is_locked(until - 1) && !mine.is_locked(until));
    assert_eq!(all["mainnetGrowi"].withdrawable_now(until), Dec::ZERO, "no follower asked about");

    let equities = equities();
    let follower = &equities["testnetHlpFollower"][0];
    assert_eq!((follower.vault_address, follower.equity), (TESTNET_HLP, dec("1068.95511")));
    assert_eq!(follower.withdrawable(1_765_926_140_668), dec("1068.95511"));
    assert_eq!(follower.withdrawable(1_765_926_140_667), Dec::ZERO);
    let two = &equities["testnetTwoVaults"];
    assert_eq!(two.len(), 2);
    assert_eq!(two[0].withdrawable(1_791_450_000_000), dec("0.000492"));
}
