//! `mainnet.json` and `testnet.json` are live answers read on 2026-10-08, cut
//! to their first rows, as the venue wrote them:
//!
//! - mainnet: a HIP-3 deployer's stake (`0x88806a71d74ad0a510b350545c9ae490912f0888`),
//!   daily rewards, one withdrawal through the queue;
//! - testnet: two validators, and validator `bob node`'s own stake
//!   (`0x0594f60525fc6a5d05bb711fb6450f84fef307bd`), hourly rewards, two
//!   withdrawals through the queue.
//!
//! The figures each test expects were worked out from the recorded rows by
//! hand, not by this code. The four reads through a client are in `tests/mocked/earn.rs`.

use crate::client::Network;
use crate::utils::Dec;
use serde::Deserialize;

use super::*;
use crate::earn::staking::reads::{
    Delegation, DelegatorSummary, MAINNET_DELEGATION_LOCK_MS, MAINNET_UNSTAKING_MS, Reward, StakeDelta, StakeEvent,
    TESTNET_DELEGATION_LOCK_MS, TESTNET_UNSTAKING_MS, delegation_lock_ms, locked_until, unstaking_ms,
};

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/stake/testnet.json"));
const DAY: u64 = 86_400_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recorded {
    delegator_summary: DelegatorSummary,
    delegations: Vec<Delegation>,
    delegator_rewards: Vec<Reward>,
    delegator_history: Vec<StakeEvent>,
}

fn recorded(network: Network) -> Recorded {
    serde_json::from_str(match network {
        Network::Mainnet => MAINNET,
        Network::Testnet => TESTNET,
    })
    .unwrap()
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// When each initiated withdrawal in the history was finalized: the next
/// newer `finalized` row.
fn queued(history: &[StakeEvent]) -> Vec<(u64, u64)> {
    history
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(&event.delta, StakeDelta::Withdrawal { phase, .. } if phase == "initiated"))
        .filter_map(|(at, event)| {
            let finalized = history[..at]
                .iter()
                .rev()
                .find(|newer| matches!(&newer.delta, StakeDelta::Withdrawal { phase, .. } if phase == "finalized"))?;
            Some((event.time, finalized.time))
        })
        .collect()
}

/// Seven days in the unstaking queue and a day's delegation lock on mainnet; five minutes and two
/// minutes on testnet. Each recorded withdrawal was finalized within a second of its queue's end,
/// and each live delegation's lock ends exactly that long after the last delegation to it.
#[test]
fn each_network_s_queue_and_lock_are_what_its_history_shows() {
    assert_eq!((MAINNET_UNSTAKING_MS, TESTNET_UNSTAKING_MS), (7 * DAY, 300_000));
    assert_eq!((MAINNET_DELEGATION_LOCK_MS, TESTNET_DELEGATION_LOCK_MS), (DAY, 120_000));
    for network in Network::ALL {
        let recorded = recorded(network);
        let spans = queued(&recorded.delegator_history);
        assert!(!spans.is_empty(), "{network}");
        for (initiated, finalized) in spans {
            let ready = initiated + unstaking_ms(network);
            assert!(ready <= finalized && finalized - ready < 1_000, "{network}: {initiated} to {finalized}");
        }
        let mut checked = 0;
        for delegation in &recorded.delegations {
            let last = recorded.delegator_history.iter().find(|event| {
                matches!(&event.delta, StakeDelta::Delegate { validator, is_undelegate: false, .. }
                    if *validator == delegation.validator)
            });
            let Some(last) = last else { continue };
            assert_eq!(delegation.locked_until_timestamp, last.time + delegation_lock_ms(network), "{network}");
            assert_eq!(locked_until(network, last.time), delegation.locked_until_timestamp, "{network}");
            assert!(delegation.is_locked(last.time) && !delegation.is_locked(delegation.locked_until_timestamp));
            checked += 1;
        }
        assert!(checked > 0, "{network}: a delegation and its row are on file");
    }
}

#[test]
fn a_withdrawal_is_pending_from_its_start_until_its_queue_ends() {
    let mainnet = recorded(Network::Mainnet).delegator_history;
    let started = 1_787_996_509_903;
    let pending = pending_withdrawals(Network::Mainnet, &mainnet, started + 3 * DAY);
    assert_eq!(
        pending,
        [PendingWithdrawal { amount: dec("500000"), initiated: started, ready_at: started + 7 * DAY }],
        "four of seven days left"
    );
    assert_eq!(pending_withdrawals(Network::Mainnet, &mainnet, started - 1), [], "not yet asked for");
    assert_eq!(pending_withdrawals(Network::Mainnet, &mainnet, 1_788_601_310_048), [], "finalized");

    let testnet = recorded(Network::Testnet).delegator_history;
    let started = 1_788_524_534_578;
    let pending = pending_withdrawals(Network::Testnet, &testnet, started + 60_000);
    assert_eq!(pending.len(), 1);
    assert_eq!((pending[0].amount, pending[0].ready_at), (dec("10000"), started + 300_000));
    assert_eq!(pending_withdrawals(Network::Testnet, &testnet, 1_788_524_835_009), [], "finalized in five minutes");
    assert_eq!(
        pending_withdrawals(Network::Mainnet, &testnet, 1_788_524_835_009).len(),
        1,
        "mainnet's queue would still hold it"
    );
}

/// The month's rewards, summed exactly: thirty daily rows on mainnet, the
/// last three hourly rows on testnet.
#[test]
fn rewards_are_summed_exactly_over_a_window() {
    let mainnet = recorded(Network::Mainnet).delegator_rewards;
    let now = 1_791_450_000_000;
    assert_eq!(rewards_since(&mainnet, now - REWARDS_WINDOW_MS), Some(dec("909.24093562")));
    let testnet = recorded(Network::Testnet).delegator_rewards;
    assert_eq!(rewards_since(&testnet, 1_788_404_400_015), Some(dec("885.76419239")), "a row at the start counts");
    assert_eq!(rewards_since(&testnet, u64::MAX), Some(Dec::ZERO));
}

/// The summary, every delegation, the newest pending withdrawals the summary
/// counts, and a month's rewards.
#[test]
fn a_whole_staking_reads_from_four_answers() {
    let recorded = recorded(Network::Mainnet);
    let started = 1_787_996_509_903;
    let mut summary = recorded.delegator_summary.clone();
    summary.n_pending_withdrawals = 1;
    let staking = Staking::new(
        Network::Mainnet,
        summary,
        recorded.delegations.clone(),
        &recorded.delegator_rewards,
        &recorded.delegator_history,
        started + DAY,
    );
    assert_eq!(staking.pending.len(), 1);
    assert_eq!(staking.delegations.len(), 2);
    assert_eq!(staking.summary.delegated, dec("501216.46877001"));
    assert_eq!(staking.total(), Some(dec("501216.46877001")));
    let none = Staking::new(
        Network::Mainnet,
        recorded.delegator_summary.clone(),
        recorded.delegations.clone(),
        &recorded.delegator_rewards,
        &recorded.delegator_history,
        started + DAY,
    );
    assert!(none.pending.is_empty(), "the summary counts none, so none is pending");
    let delegated = recorded.delegations.iter().try_fold(Dec::ZERO, |sum, one| sum.checked_add(one.amount));
    assert_eq!(delegated, Some(recorded.delegator_summary.delegated), "the delegations add up to the summary");
}
