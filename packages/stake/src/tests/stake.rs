//! The action vectors were made by `@nktkas/hyperliquid` 0.33.2 itself, as
//! hip4-tui recorded them: its own action functions, the key below, nonce
//! 1790412105001. Digests are viem's `hashTypedData` over the SDK's own
//! EIP-712 types. Mainnet.

use hl_core::{LocalKey, UserAction, digest_of_wallet_json, hex};

use super::*;
use crate::actions::{CDeposit, CWithdraw, TokenDelegate, hype_wei};

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_790_412_105_001;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

/// `(action, posted json, digest, r, s, v)`.
type Vector = (Box<dyn UserAction>, &'static str, &'static str, &'static str, &'static str, u8);

#[test]
fn every_staking_action_is_the_sdk_s_byte_for_byte() {
    let key = LocalKey::from_hex(KEY).unwrap();
    let validator = address("0x000000000056f99d36b6f2e0c51fd41496bbacb8");
    let cases: [Vector; 3] = [
        (
            Box::new(CDeposit::new(dec("100"), NONCE).unwrap()),
            r#"{"type":"cDeposit","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","wei":10000000000,"nonce":1790412105001}"#,
            "0xe9028833624165f8c7be25dd0648b988d785c4868a69f246014fa0a3b7e26cbe",
            "0xc752399304fc5710ec176378f205ecb80f9328d6b38c9b53e9390a7208545742",
            "0x124f5cce673bdf9c96dda1c02c81ac31217530b887b1f2eaa63ca92fb9f25c9f",
            28,
        ),
        (
            Box::new(CWithdraw::new(dec("1.5"), NONCE).unwrap()),
            r#"{"type":"cWithdraw","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","wei":150000000,"nonce":1790412105001}"#,
            "0xbe21b1ffb1fb8db3fa86b46af3b85e9d670328d2b037500aeff4bd9193f97f99",
            "0x6deb112b9bcf76957078d90b6a01c87bd4014da1962aa32aa933c2425ecc2b2a",
            "0x4170b2d08d6c116c3c8a3edf92591c5d9bbbc474b85d7564c6f3d410a8dfa3e7",
            28,
        ),
        (
            Box::new(TokenDelegate::delegate(validator, dec("100"), NONCE).unwrap()),
            r#"{"type":"tokenDelegate","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","validator":"0x000000000056f99d36b6f2e0c51fd41496bbacb8","wei":10000000000,"isUndelegate":false,"nonce":1790412105001}"#,
            "0x79557483c9b9eb10a6c716ce4c95ef0702198e39cbbe22ca971c3fb04c39e0e7",
            "0x333c687bae155d0538ef4e5467c0be1f8c791a3707d3f8a77b4d5003cd625225",
            "0x7aad82cb325a35ef6cfb600bbd5ef5174f4cbcfa7a0f3d52a19e985141e327cd",
            27,
        ),
    ];
    for (action, json, digest, r, s, v) in cases {
        assert_eq!(action.wire(), json);
        assert_eq!(hex::encode_prefixed(&action.digest()), digest, "{json}: the digest");
        assert_eq!(digest_of_wallet_json(&action.wallet_json()), Ok(action.digest()), "{json}: wallet json");
        let signature = key.sign_digest(&action.digest()).unwrap();
        assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, v), "{json}");
        assert_eq!(
            action.body(&signature),
            format!(r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#)
        );
    }
}

#[test]
fn hype_goes_out_as_wei_and_a_finer_amount_is_refused() {
    assert_eq!(hype_wei(dec("1.5")), Ok(150_000_000));
    assert!(hype_wei(dec("0.000000001")).is_err(), "nine places");
    assert!(CDeposit::new(Dec::ZERO, NONCE).is_err());
    let validator = address("0x000000000056f99d36b6f2e0c51fd41496bbacb8");
    let back = TokenDelegate::undelegate(validator, dec("359"), NONCE).unwrap();
    assert!(back.wire().contains(r#""wei":35900000000,"isUndelegate":true"#));
}

/// Live, mainnet, 2026-10-06: the first validator, and a jailed one.
const VALIDATORS: &str = r#"[{"validator":"0x000000000056f99d36b6f2e0c51fd41496bbacb8","signer":"0x0000000008b0b558419582041f85740344ae8fde","name":"ValiDAO","description":"The People’s Validator. Zero seed oils. Yours truly. https://validao.xyz","nRecentBlocks":1,"stake":639615957522422,"isJailed":false,"unjailableAfter":null,"isActive":true,"commission":"0.04","stats":[["day",{"uptimeFraction":"1.0","predictedApr":"0.0215379275","nSamples":1440}],["week",{"uptimeFraction":"1.0","predictedApr":"0.0215379275","nSamples":10080}],["month",{"uptimeFraction":"0.9990277553","predictedApr":"0.0215169873","nSamples":43199}]]},{"validator":"0x68e6b8995d0ea3ebe829bd66e9437eaf536aff9c","signer":"0x68e6b8995d0ea3ebe829bd66e9437eaf536aff9c","name":"HyperCN X hlscan","description":"CN community Explorer & RPC","nRecentBlocks":0,"stake":10300000000,"isJailed":true,"unjailableAfter":0,"isActive":false,"commission":"0.01","stats":[["day",{"uptimeFraction":"0.0","predictedApr":"0.0","nSamples":1440}],["week",{"uptimeFraction":"0.0","predictedApr":"0.0","nSamples":10080}],["month",{"uptimeFraction":"0.0","predictedApr":"0.0","nSamples":43199}]]}]"#;
/// Live, the same day, one HIP-3 deployer's stake: its summary, its
/// delegations, two reward rows and four history rows.
const SUMMARY: &str =
    r#"{"delegated":"501155.89935779","undelegated":"0.0","totalPendingWithdrawal":"0.0","nPendingWithdrawals":0}"#;
const DELEGATIONS: &str = r#"[{"validator":"0xa23b4556090260828ff3f939d2dbdd4f318b5f1f","amount":"270201.82202168","lockedUntilTimestamp":1783210704620},{"validator":"0xd6a72f04b9868d5d6050376d5d7b729f47305cec","amount":"230954.07733611","lockedUntilTimestamp":1788055880360}]"#;
const REWARDS: &str = r#"[{"time":1791244800027,"source":"delegation","totalAmount":"30.51325415"},{"time":1791158400121,"source":"delegation","totalAmount":"30.48969372"}]"#;
const HISTORY: &str = r#"[{"time":1788601310048,"hash":"0x0000000000000000000000000000000000000000000000000000000000000000","delta":{"withdrawal":{"amount":"500000.0","phase":"finalized"}}},{"time":1787996509903,"hash":"0xe4578d6807239e17e5d10443399db901bd00a54da226bce9882038bac6277802","delta":{"withdrawal":{"amount":"500000.0","phase":"initiated"}}},{"time":1787995389734,"hash":"0xa61684eda8389f3aa79004433961ec0201b700d3433bbe0c49df3040673c7925","delta":{"delegate":{"validator":"0xa23b4556090260828ff3f939d2dbdd4f318b5f1f","amount":"500000.0","isUndelegate":true}}},{"time":1787969170846,"hash":"0x698e1e1117edd5506b07044333d4ca01940035f6b2e0f4220d56c963d6e1af3b","delta":{"cDeposit":{"amount":"230430.0"}}}]"#;

#[test]
fn validators_read_with_their_stake_in_wei_and_stats_by_window() {
    let validators: Vec<ValidatorSummary> = serde_json::from_str(VALIDATORS).unwrap();
    let validao = &validators[0];
    assert_eq!(validao.stake_hype(), Some(dec("6396159.57522422")));
    assert_eq!(validao.commission, dec("0.04"));
    assert_eq!(validao.stats("week").map(|week| week.predicted_apr), Some(dec("0.0215379275")));
    assert_eq!(validao.unjailable_after, None);
    let jailed = &validators[1];
    assert!(jailed.is_jailed && !jailed.is_active);
    assert_eq!(jailed.unjailable_after, Some(0));
}

#[test]
fn a_delegator_s_summary_delegations_and_rewards_read() {
    let summary: DelegatorSummary = serde_json::from_str(SUMMARY).unwrap();
    assert_eq!((summary.delegated, summary.n_pending_withdrawals), (dec("501155.89935779"), 0));
    let delegations: Vec<Delegation> = serde_json::from_str(DELEGATIONS).unwrap();
    let total = delegations.iter().try_fold(Dec::ZERO, |sum, delegation| sum.checked_add(delegation.amount));
    assert_eq!(total, Some(summary.delegated), "the delegations add up to the summary");
    assert!(delegations[1].is_locked(1_788_055_880_359) && !delegations[1].is_locked(1_788_055_880_360));
    let rewards: Vec<Reward> = serde_json::from_str(REWARDS).unwrap();
    assert_eq!((rewards[0].source.as_str(), rewards[0].total_amount), ("delegation", dec("30.51325415")));
}

#[test]
fn history_rows_read_by_kind_and_an_initiated_withdrawal_lands_after_the_queue() {
    let history: Vec<StakeEvent> = serde_json::from_str(HISTORY).unwrap();
    assert_eq!(history[0].delta, StakeDelta::Withdrawal { amount: dec("500000"), phase: "finalized".to_owned() });
    assert_eq!(history[3].delta, StakeDelta::CDeposit { amount: dec("230430") });
    assert_eq!(
        history[2].delta,
        StakeDelta::Delegate {
            validator: address("0xa23b4556090260828ff3f939d2dbdd4f318b5f1f"),
            amount: dec("500000"),
            is_undelegate: true,
        }
    );
    let landed = history[1].unstaked_at().unwrap();
    assert!(landed <= history[0].time && history[0].time - landed < 1_000, "finalized a breath after seven days");
    assert_eq!(history[0].unstaked_at(), None);
    let unknown: Vec<StakeEvent> =
        serde_json::from_str(r#"[{"time":1,"hash":"0x01","delta":{"slash":{"amount":"1.0"}}}]"#).unwrap();
    assert_eq!(unknown[0].delta, StakeDelta::Other { kind: "slash".to_owned() });
}

#[test]
fn the_reads_name_their_type_and_user() {
    let user = address("0x88806A71D74AD0A510B350545C9AE490912F0888");
    assert_eq!(
        delegator_history_request(user),
        r#"{"type":"delegatorHistory","user":"0x88806a71d74ad0a510b350545c9ae490912f0888"}"#
    );
    assert_eq!(validator_summaries_request(), r#"{"type":"validatorSummaries"}"#);
    assert_eq!((DELEGATION_LOCK_MS, UNSTAKING_MS), (86_400_000, 604_800_000));
}
