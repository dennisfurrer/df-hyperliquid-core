//! Every staking vector in `vectors/nktkas-0.33.2.json` (`@nktkas/hyperliquid`
//! 0.33.2), through this crate's constructors and plans, on the network each
//! case was made for: the posted JSON, the digest and the signature.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use df_hyperliquid_core::account::{LocalKey, Signers, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::earn::staking::{CDeposit, CWithdraw, Ctx, DelegatorSummary, TokenDelegate};
use df_hyperliquid_core::utils::sign::{UserAction, digest_of_wallet_json};
use df_hyperliquid_core::utils::{Address, Dec, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));

const NAMES: [&str; 3] = ["cDeposit", "cWithdraw", "tokenDelegate"];
const VALIDATOR: &str = "0x000000000056f99d36b6f2e0c51fd41496bbacb8";

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn network(case: &Json) -> Network {
    match case.get("isTestnet") {
        Some(Json::Bool(true)) => Network::Testnet,
        Some(Json::Bool(false)) => Network::Mainnet,
        other => panic!("isTestnet is {other:?}"),
    }
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn validator() -> Address {
    VALIDATOR.parse().unwrap()
}

/// From typed HYPE, the amounts the vectors were made with.
fn built(name: &str, nonce: u64) -> Box<dyn UserAction> {
    match name {
        "cDeposit" => Box::new(CDeposit::new(dec("100"), nonce).unwrap()),
        "cWithdraw" => Box::new(CWithdraw::new(dec("1.5"), nonce).unwrap()),
        "tokenDelegate" => Box::new(TokenDelegate::delegate(validator(), dec("100"), nonce).unwrap()),
        other => panic!("no such action {other}"),
    }
}

/// The plans make the same actions, from a staking balance that holds enough.
fn planned(name: &str, network: Network, nonce: u64) -> Box<dyn UserAction> {
    let ctx = Ctx { network, nonce, now_ms: nonce };
    let summary = DelegatorSummary {
        delegated: Dec::ZERO,
        undelegated: dec("1000"),
        total_pending_withdrawal: Dec::ZERO,
        n_pending_withdrawals: 0,
    };
    let plan_key = |key: Signers| assert_eq!(key, Signers::Main, "{name}: user-signed");
    match name {
        "cDeposit" => {
            let plan = ctx.stake(dec("100")).unwrap();
            plan_key(plan.key);
            Box::new(plan.action)
        }
        "cWithdraw" => {
            let plan = ctx.unstake(dec("1.5"), &summary).unwrap();
            plan_key(plan.key);
            Box::new(plan.action)
        }
        "tokenDelegate" => {
            let plan = ctx.delegate(validator(), dec("100"), &summary).unwrap();
            plan_key(plan.key);
            Box::new(plan.action)
        }
        other => panic!("no such action {other}"),
    }
}

#[test]
fn every_nktkas_staking_action_signs_on_mainnet_and_testnet() {
    let file = Json::parse(NKTKAS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("userSigned")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| NAMES.contains(&text(case, "name")))
        .collect();
    assert_eq!(cases.len(), 6, "three actions, each on both networks");
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 3);
    for case in cases {
        let (name, network) = (text(case, "name"), network(case));
        let json = case.get("action").unwrap().compact();
        for action in [built(name, nonce), planned(name, network, nonce)] {
            assert_eq!(action.wire(network), json, "{name} on {network}: the posted action");
            let digest = action.digest(network);
            assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{name} on {network}: the digest");
            assert_eq!(digest_of_wallet_json(&action.wallet_json(network)), Ok(digest), "{name} on {network}");
            let signature = key.sign_digest(&digest).unwrap();
            let expected = case.get("signature").unwrap();
            let v = expected.get("v").and_then(Json::as_u64).unwrap();
            assert_eq!(
                (signature.r_hex().as_str(), signature.s_hex().as_str(), u64::from(signature.v())),
                (text(expected, "r"), text(expected, "s"), v),
                "{name} on {network}: the signature"
            );
            assert_eq!(recover(&digest, &signature), Ok(key.address()), "{name} on {network}");
            let (r, s) = (text(expected, "r"), text(expected, "s"));
            assert_eq!(
                action.body(&signature, network),
                format!(r#"{{"action":{json},"nonce":{nonce},"signature":{{"r":"{r}","s":"{s}","v":{v}}}}}"#),
                "{name} on {network}: the body"
            );
        }
    }
}
