//! `@nktkas/hyperliquid` 0.33.2's vectors (`vectors/nktkas-0.33.2.json`) through the signing
//! code alone: each L1 action exactly as written (key order kept), its hash and its signature on
//! the network it was made for, and a user-signed action on both networks. The other L1 files are
//! rebuilt through the typed builders in `trade/`.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::utils::sign::user_signed::NonceKey;
use df_hyperliquid_core::utils::sign::{L1Action, UserSigned};
use df_hyperliquid_core::utils::{Address, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));

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

/// Some files write `r` and `s` without leading zeros.
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

/// `usdClassTransfer` on both networks, built by hand: the chain names, the digest and the
/// signature all follow the network. The rest of the user-signed vectors run through the modules
/// that build those actions.
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
                .field("amount", df_hyperliquid_core::utils::sign::Field::String("12.5"))
                .field("toPerp", df_hyperliquid_core::utils::sign::Field::Bool(true));
        assert_eq!(action.wire(network), case.get("action").unwrap().compact(), "{network}: wire");
        let digest = action.digest(network);
        assert_eq!(hex::encode_prefixed(&digest), text(case, "signingHash"), "{network}: digest");
        assert_eq!(
            df_hyperliquid_core::utils::sign::eip712::digest_of_wallet_json(&action.wallet_json(network)),
            Ok(digest),
            "{network}"
        );
        let signature = key.sign_digest(&digest).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), text(expected, "r"), "{network}: r");
        assert_eq!(signature.s_hex(), text(expected, "s"), "{network}: s");
    }
}
