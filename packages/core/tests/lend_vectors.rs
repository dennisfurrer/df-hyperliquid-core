//! Every `borrowLend` vector in `vectors/nktkas-0.33.2.json`
//! (`@nktkas/hyperliquid` 0.33.2), through this crate's constructors, on the
//! network each case was made for: the posted JSON, the connection id and the
//! signature. The SDK run named HYPE by its mainnet index, 150, on both
//! networks; the signature does not care.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use df_hyperliquid_core::account::{LocalKey, Signers, recover};
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::earn::lending::{BorrowLend, Ctx, ReserveState};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Dec, Json, hex};

const NKTKAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"));
/// Live `allBorrowLendReserveStates`, both networks, 2026-10-08.
const RESERVES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/lend/reserves.json"));

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

fn reserves(network: Network) -> Vec<(u32, ReserveState)> {
    let all: serde_json::Value = serde_json::from_str(RESERVES).unwrap();
    serde_json::from_value(all[network.name()].clone()).unwrap()
}

#[test]
fn every_nktkas_borrow_lend_hashes_and_signs_on_mainnet_and_testnet() {
    let file = Json::parse(NKTKAS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases: Vec<&Json> = file
        .get("l1")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .filter(|case| text(case, "name") == "borrowLend")
        .collect();
    assert_eq!(cases.len(), 4, "a supply and a whole repay, each on both networks");
    assert_eq!(cases.iter().filter(|case| network(case) == Network::Testnet).count(), 2);
    for case in cases {
        let network = network(case);
        let wire = case.get("action").unwrap();
        let action = match text(wire, "operation") {
            "supply" => BorrowLend::supply(0, Dec::parse("100.50").unwrap()).unwrap(),
            "repay" => BorrowLend::repay_all(150),
            other => panic!("no such case {other}"),
        };
        let json = wire.compact();
        assert_eq!(action.wire(), json, "{network}: the posted action");
        assert_eq!(hex::encode_prefixed(&action.connection_id(nonce, None)), text(case, "actionHash"), "{json}");
        let digest = action.digest(nonce, None, network);
        let signature = key.sign_digest(&digest).unwrap();
        let expected = case.get("signature").unwrap();
        let v = expected.get("v").and_then(Json::as_u64).unwrap();
        let (r, s) = (text(expected, "r"), text(expected, "s"));
        assert_eq!(
            (signature.r_hex().as_str(), signature.s_hex().as_str(), u64::from(signature.v())),
            (r, s, v),
            "{json} on {network}: the signature"
        );
        assert_eq!(recover(&digest, &signature), Ok(key.address()), "{json} on {network}");
        assert_eq!(
            action.body(nonce, &signature, None),
            format!(
                r#"{{"action":{json},"nonce":{nonce},"signature":{{"r":"{r}","s":"{s}","v":{v}}},"vaultAddress":null}}"#
            )
        );
        if text(wire, "operation") == "supply" {
            let plan = Ctx { network, nonce }.supply(0, Dec::parse("100.5").unwrap(), &reserves(network)).unwrap();
            assert_eq!((plan.action, plan.nonce, plan.key), (action, nonce, Signers::Either), "{network}: the plan");
        }
    }
}
