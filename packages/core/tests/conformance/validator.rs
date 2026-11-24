//! `validator_actions.json`: `CValidatorAction` and `CSignerAction`, each built through the typed
//! actions from the inputs the reference SDK was given, then held to its posted JSON, msgpack, hash
//! and mainnet signature. Two sources, `@nktkas/hyperliquid` 0.33.3 and Hyperliquid's Python SDK
//! 0.24.0, which agree on every case.

use std::collections::BTreeMap;

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::earn::validator::{CSignerAction, CValidatorAction, ProfileChange, ValidatorProfile};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Address, Json, hex};

const VECTORS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/validator_actions.json"));

const USER: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
const USER2: &str = "0x2222222222222222222222222222222222222222";

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn profile(ip: &str) -> ValidatorProfile {
    ValidatorProfile {
        node_ip: ip.parse().unwrap(),
        name: "Test validator".to_owned(),
        description: "A test.".to_owned(),
        delegations_disabled: true,
        commission_bps: 500,
        signer: address(USER),
    }
}

/// The case's action, built from the inputs its SDK was given.
fn built(name: &str) -> Box<dyn L1Action> {
    match name {
        "register" => Box::new(CValidatorAction::Register {
            profile: profile("1.2.3.4"),
            unjailed: false,
            initial_wei: 1_000_000_000_000,
        }),
        "registerIpv6" => {
            Box::new(CValidatorAction::Register { profile: profile("2001:db8::1"), unjailed: true, initial_wei: 0 })
        }
        "changeProfileUnjailOnly" => {
            Box::new(CValidatorAction::ChangeProfile(ProfileChange { unjailed: true, ..ProfileChange::default() }))
        }
        "changeProfileEverything" => Box::new(CValidatorAction::ChangeProfile(ProfileChange {
            node_ip: Some("5.6.7.8".parse().unwrap()),
            name: Some("Renamed".to_owned()),
            description: Some(String::new()),
            unjailed: false,
            disable_delegations: Some(false),
            commission_bps: Some(1000),
            signer: Some(address(USER2)),
        })),
        "unregister" => Box::new(CValidatorAction::Unregister),
        "jailSelf" => Box::new(CSignerAction::JailSelf),
        "unjailSelf" => Box::new(CSignerAction::UnjailSelf),
        other => panic!("no such case {other}"),
    }
}

/// The Python SDK writes `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

/// Every case of `section` against its typed build; returns each case's hash by name.
fn check_section(section: &str) -> BTreeMap<String, String> {
    let file = Json::parse(VECTORS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases = file.get(section).and_then(|one| one.get("cases")).and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 7, "{section}");
    let mut hashes = BTreeMap::new();
    for case in cases {
        let name = text(case, "name");
        let typed = built(name);
        let recorded = case.get("action").unwrap();
        assert_eq!(typed.wire(), recorded.compact(), "{section} {name}: the action");
        assert_eq!(typed.msgpack(), recorded.to_action().unwrap().msgpack(), "{section} {name}: msgpack of the JSON");
        if let Some(Json::String(packed)) = case.get("msgpack") {
            assert_eq!(hex::encode_prefixed(&typed.msgpack()), *packed, "{section} {name}: msgpack");
        }
        let hash = hex::encode_prefixed(&typed.connection_id(nonce, None));
        assert_eq!(hash, text(case, "actionHash"), "{section} {name}: hash");
        let signature = key.sign_digest(&typed.digest(nonce, None, Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), word(text(expected, "r")), "{section} {name}: r");
        assert_eq!(signature.s_hex(), word(text(expected, "s")), "{section} {name}: s");
        assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{section} {name}: v");
        hashes.insert(name.to_owned(), hash);
    }
    hashes
}

#[test]
fn both_sdks_validator_and_signer_actions_come_out_of_the_builders_with_the_same_hashes() {
    let nktkas = check_section("nktkas");
    let python = check_section("python");
    assert_eq!(nktkas, python, "the two SDKs hash every case alike");
}
