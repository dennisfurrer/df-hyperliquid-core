//! The staking answers as the venue writes them: validators in `validator_summaries_mainnet.json`
//! (live, 2026-10-18: an active one, a jailed one, one inactive) and testnet's two in
//! `testnet.json`; history rows in `mainnet.json`, each read by its kind. The summary, delegations
//! and rewards are read whole in `tests/staking.rs`; the staking action vectors are in
//! `tests/conformance/stake.rs`.

use crate::client::Network;
use crate::utils::Dec;

use super::*;

const VALIDATORS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/validator_summaries_mainnet.json"));
const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/testnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn named<'a>(validators: &'a [ValidatorSummary], name: &str) -> &'a ValidatorSummary {
    validators.iter().find(|one| one.name == name).unwrap()
}

/// A validator's stake is wei, 1e8 to a HYPE; its stats come by window; a jailed one says when it
/// may be unjailed, and an active one writes `null` there.
#[test]
fn validators_read_their_stake_in_wei_and_their_stats_by_window() {
    let mainnet: Vec<ValidatorSummary> = serde_json::from_str(VALIDATORS).unwrap();
    let validao = named(&mainnet, "ValiDAO");
    assert_eq!(validao.validator, address("0x000000000056f99d36b6f2e0c51fd41496bbacb8"));
    assert_eq!(validao.stake_hype(), Some(dec("6499721.03992442")));
    assert_eq!(validao.commission, dec("0.04"));
    assert_eq!(validao.stats("week").map(|week| week.predicted_apr), Some(dec("0.0214214476")));
    assert_eq!(validao.stats("year"), None);
    assert!(validao.is_active && !validao.is_jailed && validao.unjailable_after.is_none());
    let jailed = named(&mainnet, "HyperCN X hlscan");
    assert!(jailed.is_jailed && !jailed.is_active);
    assert_eq!(jailed.unjailable_after, Some(0));
    let resting = named(&mainnet, "Meria");
    assert!(!resting.is_active && !resting.is_jailed);

    let testnet: Vec<ValidatorSummary> = serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(TESTNET).unwrap()["validatorSummaries"].clone(),
    )
    .unwrap();
    let hypurr = named(&testnet, "Hypurr2");
    assert_eq!(hypurr.stake_hype(), Some(dec("3210070.54148517")));
    assert_eq!(hypurr.stats("week").map(|week| week.predicted_apr), Some(dec("0.0329726982")));
    assert!(named(&testnet, "somebody").commission == Dec::ONE);
}

/// Each row is keyed by its one field's name; a kind this build does not read, or a known kind
/// that no longer reads as modelled, is kept by name rather than failing the history.
#[test]
fn history_rows_read_by_their_kind_and_any_other_is_kept_by_name() {
    let recorded: serde_json::Value = serde_json::from_str(MAINNET).unwrap();
    let history: Vec<StakeEvent> = serde_json::from_value(recorded["delegatorHistory"].clone()).unwrap();
    assert_eq!(history[0].delta, StakeDelta::Withdrawal { amount: dec("500000"), phase: "finalized".to_owned() });
    assert_eq!(
        history[2].delta,
        StakeDelta::Delegate {
            validator: address("0xa23b4556090260828ff3f939d2dbdd4f318b5f1f"),
            amount: dec("500000"),
            is_undelegate: true,
        }
    );
    assert_eq!(history[5].delta, StakeDelta::CDeposit { amount: dec("230430") });
    assert_eq!(history[1].unstaked_at(Network::Mainnet), Some(history[1].time + MAINNET_UNSTAKING_MS));
    assert_eq!(history[0].unstaked_at(Network::Mainnet), None, "only an initiated withdrawal is on its way");
    let other: Vec<StakeEvent> = serde_json::from_str(concat!(
        r#"[{"time":1,"hash":"0x01","delta":{"slash":{"amount":"1.0"}}},"#,
        r#"{"time":2,"hash":"0x02","delta":{"cDeposit":{"amount":"lots"}}}]"#
    ))
    .unwrap();
    assert_eq!(other[0].delta, StakeDelta::Other { kind: "slash".to_owned() });
    assert_eq!(other[1].delta, StakeDelta::Other { kind: "cDeposit".to_owned() });
}
