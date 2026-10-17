//! Every `vaultTransfer` vector in `vectors/nktkas-0.33.2.json`
//! (`@nktkas/hyperliquid` 0.33.2), through this crate's constructor and plan,
//! on the network each case was made for: the posted JSON, the connection id
//! and the signature. The SDK run named mainnet's HLP on both networks; the
//! signature does not care.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use df_hyperliquid_core::account::{LocalKey, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::earn::vaults::{Ctx, Key, MAINNET_HLP, VaultDetails, VaultTransfer};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Dec, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
/// Live `vaultDetails`, mainnet HLP, 2026-10-08.
const DETAILS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vault/details.json"));

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

#[test]
fn every_nktkas_vault_transfer_hashes_and_signs_on_mainnet_and_testnet() {
    let file = Json::parse(NKTKAS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("l1")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| text(case, "name") == "vaultTransfer")
        .collect();
    assert_eq!(cases.len(), 2, "one deposit on each network");
    let all: serde_json::Value = serde_json::from_str(DETAILS).unwrap();
    let hlp: VaultDetails = serde_json::from_value(all["mainnetHlp"].clone()).unwrap();
    for case in cases {
        let network = network(case);
        let action = VaultTransfer::deposit(MAINNET_HLP, Dec::parse("10").unwrap()).unwrap();
        let json = case.get("action").unwrap().compact();
        assert_eq!(action.wire(), json, "{network}: the posted action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{network}");
        let digest = action.digest(nonce, None, network);
        let signature = key.sign_digest(&digest).unwrap();
        let expected = case.get("signature").unwrap();
        let v = expected.get("v").and_then(Json::as_u64).unwrap();
        let (r, s) = (text(expected, "r"), text(expected, "s"));
        assert_eq!(
            (signature.r_hex().as_str(), signature.s_hex().as_str(), u64::from(signature.v())),
            (r, s, v),
            "{network}: the signature"
        );
        assert_eq!(recover(&digest, &signature), Ok(key.address()), "{network}");
        assert_eq!(
            action.body(nonce, &signature, None),
            format!(
                r#"{{"action":{json},"nonce":{nonce},"signature":{{"r":"{r}","s":"{s}","v":{v}}},"vaultAddress":null}}"#
            )
        );
        let plan = Ctx { network, nonce, now_ms: nonce }.deposit(&hlp, Dec::parse("10").unwrap()).unwrap();
        assert_eq!((plan.action, plan.nonce, plan.key), (action, nonce, Key::Either), "{network}: the plan");
    }
}
