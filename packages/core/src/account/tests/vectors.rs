//! The account's actions signed as other implementations sign them: `@nktkas/hyperliquid` 0.33.2's
//! vectors on both networks, its 0.24.3 approvals, a session made by hand, and a second
//! implementation's subaccount hashes. Each case is built through this crate's typed builders from
//! what the case says, then its posted action, hash or digest, signature and body are compared.

use crate::account::{
    AbstractionMode, AgentSetAbstraction, ApproveAgent, ApproveBuilderFee, CreateSubAccount, LocalKey, SetReferrer,
    Signature, SubAccountSpotTransfer, SubAccountTransfer, UserSetAbstraction, recover,
};
use crate::client::Network;
use crate::trade::builder::fee::BuilderFee;
use crate::utils::sign::{L1Action, UserAction, digest_of_wallet_json};
use crate::utils::{Address, Dec, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
/// `@nktkas/hyperliquid` 0.24.3, the unnamed agent posted as null.
const USER_SIGNED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/user_signed.json"));
/// A session made by hand, the owner's two approvals.
const SESSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/session.json"));

const OURS: [&str; 8] = [
    "setReferrer",
    "agentSetAbstraction",
    "subAccountTransfer",
    "subAccountSpotTransfer",
    "createSubAccount",
    "approveAgent",
    "approveBuilderFee",
    "userSetAbstraction",
];

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn number(json: &Json, key: &str) -> u64 {
    json.get(key).and_then(Json::as_u64).unwrap_or_else(|| panic!("no {key}"))
}

fn yes(json: &Json, key: &str) -> bool {
    match json.get(key) {
        Some(Json::Bool(value)) => *value,
        other => panic!("{key} is {other:?}"),
    }
}

fn network(case: &Json) -> Network {
    match yes(case, "isTestnet") {
        true => Network::Testnet,
        false => Network::Mainnet,
    }
}

fn signed(expected: &Json, signature: &Signature, name: &str) {
    assert_eq!(signature.r_hex(), text(expected, "r"), "{name}: r");
    assert_eq!(signature.s_hex(), text(expected, "s"), "{name}: s");
    assert_eq!(u64::from(signature.v()), number(expected, "v"), "{name}: v");
}

/// A user-signed case, built from what the case says.
fn user_signed(action: &Json) -> Box<dyn UserAction> {
    let nonce = number(action, "nonce");
    match text(action, "type") {
        "approveAgent" => {
            let agent = text(action, "agentAddress").parse().unwrap();
            match action.get("agentName") {
                Some(Json::Null) => Box::new(ApproveAgent::unnamed(agent, nonce)),
                _ => Box::new(ApproveAgent::new(agent, text(action, "agentName"), nonce)),
            }
        }
        "approveBuilderFee" => Box::new(ApproveBuilderFee::new(
            text(action, "builder").parse().unwrap(),
            BuilderFee::from_percent(text(action, "maxFeeRate")).unwrap(),
            nonce,
        )),
        "userSetAbstraction" => Box::new(UserSetAbstraction::new(
            text(action, "user").parse().unwrap(),
            AbstractionMode::from_wire(text(action, "abstraction")).unwrap(),
            nonce,
        )),
        other => panic!("{other} is not this crate's"),
    }
}

fn l1(action: &Json) -> Box<dyn L1Action> {
    let sub = || text(action, "subAccountUser").parse::<Address>().unwrap();
    match text(action, "type") {
        "setReferrer" => Box::new(SetReferrer::new(text(action, "code")).unwrap()),
        "agentSetAbstraction" => {
            Box::new(AgentSetAbstraction::new(AbstractionMode::from_code(text(action, "abstraction")).unwrap()))
        }
        "createSubAccount" => Box::new(CreateSubAccount::new(text(action, "name"))),
        "subAccountTransfer" => {
            let usdc = Dec::new(i128::from(number(action, "usd")), 6).unwrap();
            Box::new(match yes(action, "isDeposit") {
                true => SubAccountTransfer::deposit(sub(), usdc).unwrap(),
                false => SubAccountTransfer::withdraw(sub(), usdc).unwrap(),
            })
        }
        "subAccountSpotTransfer" => Box::new(
            SubAccountSpotTransfer::new(
                sub(),
                yes(action, "isDeposit"),
                text(action, "token"),
                Dec::parse(text(action, "amount")).unwrap(),
            )
            .unwrap(),
        ),
        other => panic!("{other} is not this crate's"),
    }
}

fn ours(file: &Json, list: &str) -> Vec<Json> {
    file.get(list)
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| OURS.contains(&text(case, "name")))
        .cloned()
        .collect()
}

#[test]
fn every_nktkas_user_signed_account_action_signs_and_posts_as_the_sdk_does_on_both_networks() {
    let file = Json::parse(NKTKAS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let cases = ours(&file, "userSigned");
    assert_eq!(cases.len(), 6, "three actions, each on both networks");
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 3);
    for case in &cases {
        let (name, network) = (text(case, "name"), network(case));
        let written = case.get("action").unwrap();
        let action = user_signed(written);
        assert_eq!(action.wire(network), written.compact(), "{name} on {network}: the posted action");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{name} on {network}: digest");
        assert_eq!(digest_of_wallet_json(&action.wallet_json(network)), Ok(digest), "{name} on {network}");
        let signature = key.sign_digest(&digest).unwrap();
        signed(case.get("signature").unwrap(), &signature, name);
        assert_eq!(recover(&digest, &signature), Ok(key.address()));
        let nonce = number(written, "nonce");
        assert_eq!(
            action.body(&signature, network),
            format!(r#"{{"action":{},"nonce":{nonce},"signature":{}}}"#, written.compact(), signature.to_json()),
            "{name} on {network}: the body"
        );
    }
}

#[test]
fn every_nktkas_l1_account_action_hashes_signs_and_posts_as_the_sdk_does_on_both_networks() {
    let file = Json::parse(NKTKAS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = number(&file, "nonce");
    let cases = ours(&file, "l1");
    assert_eq!(cases.len(), 10, "five actions, each on both networks");
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 5);
    for case in &cases {
        let (name, network) = (text(case, "name"), network(case));
        let written = case.get("action").unwrap();
        let action = l1(written);
        assert_eq!(action.wire(), written.compact(), "{name}: the posted action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{name}");
        let signature = key.sign_digest(&action.digest(nonce, None, network)).unwrap();
        signed(case.get("signature").unwrap(), &signature, &format!("{name} on {network}"));
        assert_eq!(
            action.body(nonce, &signature, None),
            format!(
                r#"{{"action":{},"nonce":{nonce},"signature":{},"vaultAddress":null}}"#,
                written.compact(),
                signature.to_json()
            ),
            "{name} on {network}: the body"
        );
    }
}

/// Approvals made with `@nktkas/hyperliquid` 0.24.3 on mainnet: a named agent, the unnamed one
/// posted with a null name, and 0.1% to a builder.
#[test]
fn every_0_24_3_approval_matches() {
    let file = Json::parse(USER_SIGNED).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    assert_eq!(key.address(), text(&file, "address").parse::<Address>().unwrap());
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let name = text(case, "name");
        let written = case.get("action").unwrap();
        let network: Network = text(written, "hyperliquidChain").parse().unwrap();
        let action = user_signed(written);
        assert_eq!(action.wire(network), written.compact(), "{name}: the posted action");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{name}: digest");
        let signature = key.sign_digest(&digest).unwrap();
        signed(case.get("signature").unwrap(), &signature, name);
    }
}

/// The session owner's approvals, to the body posted.
#[test]
fn the_session_s_approvals_post_the_body_it_recorded() {
    let file = Json::parse(SESSION).unwrap();
    let owner: Address = text(&file, "owner").parse().unwrap();
    // Hardhat's first account, the session's owner.
    let key = LocalKey::from_hex("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80").unwrap();
    assert_eq!(key.address(), owner);
    let cases: Vec<&Json> =
        file.get("cases").and_then(Json::as_array).unwrap().iter().filter(|case| text(case, "by") == "owner").collect();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let name = text(case, "name");
        let action = user_signed(case.get("action").unwrap());
        let signature = key.sign_digest(&action.digest(Network::Mainnet)).unwrap();
        signed(case.get("signature").unwrap(), &signature, name);
        assert_eq!(action.body(&signature, Network::Mainnet), text(case, "body"), "{name}: the body");
    }
}

/// `createSubAccount` and `subAccountTransfer` at nonce 1700000000000, hashes cross-checked
/// against a second implementation.
#[test]
fn subaccount_actions_hash_as_the_other_implementation_does() {
    let sub: Address = "0x1234567890abcdef1234567890abcdef12345678".parse().unwrap();
    let nonce = 1_700_000_000_000;
    let usdc = Dec::parse("2.5").unwrap();
    assert_eq!(
        hex::encode(&CreateSubAccount::new("desk two").connection_id(nonce, None)),
        "78b7bf674f2b129a45b32e7d7ccab4868fe9df798fdf45285f0275b4c34317db"
    );
    let into = SubAccountTransfer::deposit(sub, usdc).unwrap();
    assert_eq!(
        hex::encode(&into.connection_id(nonce, None)),
        "7c4729feca68abc1060366b2ae5b7742cd968e494eabf95bc051e09ce6f14094"
    );
    let out = SubAccountTransfer::withdraw(sub, usdc).unwrap();
    assert_ne!(out.connection_id(nonce, None), into.connection_id(nonce, None), "the direction is signed");
    assert_eq!(SubAccountTransfer::from_micros(sub, true, 2_500_000), into);
}
