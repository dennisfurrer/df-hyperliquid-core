//! Every L1 vector on file, through hl-core alone: the action exactly as
//! written (key order kept), its msgpack, its hash, and its signature on the
//! network it was made for. The files are in `vectors/`, each naming the
//! implementation that made it.
//!
//! - `l1.json`: `@nktkas/hyperliquid` 0.24.3, as atlas recorded it.
//! - `vault.json`: the venue's Python SDK 0.19.0, acting for a subaccount.
//! - `nktkas-0.33.2.json`: `@nktkas/hyperliquid` 0.33.2, as hip4-tui recorded it,
//!   every action on mainnet and on testnet.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use hl_core::{Address, Json, L1Action, LocalKey, Network, UserSigned, hex, user_signed::NonceKey};

const L1: &str = include_str!("../../../vectors/l1.json");
const VAULT: &str = include_str!("../../../vectors/vault.json");
const NKTKAS: &str = include_str!("../../../vectors/nktkas-0.33.2.json");

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

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

/// The Python SDK writes `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

/// One case: msgpack where the file has it, the hash, then the signature.
fn check_l1(name: &str, case: &Json, key: &LocalKey, nonce: u64) {
    let action = case.get("action").unwrap().to_action().unwrap();
    if let Some(Json::String(packed)) = case.get("msgpack") {
        assert_eq!(hex::encode_prefixed(&action.msgpack()), *packed, "{name}: msgpack");
    }
    let vault = match case.get("vaultAddress") {
        Some(Json::String(vault)) => Some(vault.parse::<Address>().unwrap()),
        _ => None,
    };
    let network = network(case);
    assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, vault)), text(case, "actionHash"), "{name}: hash");
    let signature = key.sign_digest(&action.digest(nonce, vault, network)).unwrap();
    let expected = case.get("signature").unwrap();
    assert_eq!(signature.r_hex(), word(text(expected, "r")), "{name} on {network}: r");
    assert_eq!(signature.s_hex(), word(text(expected, "s")), "{name} on {network}: s");
    assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{name}: v");
}

#[test]
fn every_atlas_l1_case_hashes_and_signs_as_recorded() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    assert_eq!(key.address().to_hex(), text(&file, "address").to_lowercase());
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 16);
    assert!(cases.iter().any(|case| network(case) == Network::Testnet), "a testnet case is on file");
    for case in cases {
        let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
        check_l1(text(case, "name"), case, &key, nonce);
    }
}

#[test]
fn the_python_sdk_s_subaccount_cases_hash_and_sign_the_same() {
    let file = parse(VAULT);
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let name = text(case, "name");
        let key = LocalKey::from_hex(text(case, "key")).unwrap();
        assert_eq!(key.address().to_hex(), text(case, "address").to_lowercase(), "{name}");
        check_l1(name, case, &key, case.get("nonce").and_then(Json::as_u64).unwrap());
    }
}

#[test]
fn every_nktkas_l1_action_signs_on_mainnet_and_testnet() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases = file.get("l1").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 16);
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 8);
    for case in cases {
        check_l1(text(case, "name"), case, &key, nonce);
    }
}

/// `usdClassTransfer` on both networks, built by hand: the chain names, the
/// digest and the signature all follow the network. The rest of the
/// user-signed vectors run through the crates that build those actions.
#[test]
fn a_user_signed_action_follows_its_network() {
    let file = parse(NKTKAS);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("userSigned")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| text(case, "name") == "usdClassTransfer")
        .collect();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let network = network(case);
        let action =
            UserSigned::new("usdClassTransfer", "HyperliquidTransaction:UsdClassTransfer", NonceKey::Nonce, nonce)
                .field("amount", hl_core::Field::String("12.5"))
                .field("toPerp", hl_core::Field::Bool(true));
        assert_eq!(action.wire(network), case.get("action").unwrap().compact(), "{network}: wire");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{network}: digest");
        assert_eq!(hl_core::eip712::digest_of_wallet_json(&action.wallet_json(network)), Ok(digest), "{network}");
        let signature = key.sign_digest(&digest).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), text(expected, "r"), "{network}: r");
        assert_eq!(signature.s_hex(), text(expected, "s"), "{network}: s");
    }
}
