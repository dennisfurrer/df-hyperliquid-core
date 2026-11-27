//! Every case in `vectors/l1.json` (made by `@nktkas/hyperliquid`), stage by stage: the msgpack
//! bytes, the action hash, the payload a wallet signs, the digest, and with the file's key the
//! signature and the address it recovers to.

use super::*;
use crate::utils::hex;
use crate::utils::json::Json;
use crate::utils::sign::eip712::digest_of_wallet_json;

const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap()
}

#[test]
fn every_l1_vector_breaks_down_to_the_recorded_bytes() {
    let file = Json::parse(L1).unwrap();
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let name = text(case, "name");
        let action = case.get("action").unwrap().to_action().unwrap();
        let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
        let vault = case.get("vaultAddress").and_then(Json::as_str).map(|vault| vault.parse::<Address>().unwrap());
        let network = if case.get("isTestnet").and_then(Json::as_bool) == Some(true) {
            Network::Testnet
        } else {
            Network::Mainnet
        };
        let stages = Breakdown::of(&action, nonce, vault, network);
        assert_eq!(hex::encode_prefixed(&stages.msgpack), text(case, "msgpack"), "{name}: msgpack");
        assert_eq!(hex::encode_prefixed(&stages.action_hash), text(case, "actionHash"), "{name}: action hash");
        assert_eq!(digest_of_wallet_json(&stages.payload), Ok(stages.digest), "{name}: payload");
        assert_eq!(stages.digest, action.digest(nonce, vault, network), "{name}: digest");
    }
}

#[cfg(feature = "sign")]
#[test]
fn the_last_stage_recovers_the_file_s_key_from_its_recorded_signatures() {
    use crate::account::LocalKey;
    let file = Json::parse(L1).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let action = case.get("action").unwrap().to_action().unwrap();
        let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
        let vault = case.get("vaultAddress").and_then(Json::as_str).map(|vault| vault.parse::<Address>().unwrap());
        let network = if case.get("isTestnet").and_then(Json::as_bool) == Some(true) {
            Network::Testnet
        } else {
            Network::Mainnet
        };
        let stages = Breakdown::of(&action, nonce, vault, network);
        let recorded = case.get("signature").unwrap();
        let signature = key.sign_digest(&stages.digest).unwrap();
        assert_eq!(signature.r_hex(), text(recorded, "r"), "{name}: r");
        assert_eq!(signature.s_hex(), text(recorded, "s"), "{name}: s");
        assert_eq!(u64::from(signature.v()), recorded.get("v").and_then(Json::as_u64).unwrap(), "{name}: v");
        assert_eq!(stages.recover(&signature), Ok(key.address()), "{name}: recovered address");
    }
}

#[test]
fn an_expiry_changes_only_the_hash_and_what_follows_it() {
    let action = crate::utils::sign::msgpack::Map::new().put("type", "noop");
    let plain = Breakdown::of(&action, 7, None, Network::Mainnet);
    let expiring = Breakdown::expiring(&action, 7, None, Some(5_000), Network::Mainnet);
    assert_eq!(plain, Breakdown::expiring(&action, 7, None, None, Network::Mainnet));
    assert_eq!(plain.msgpack, expiring.msgpack);
    assert_ne!(plain.action_hash, expiring.action_hash);
    assert_ne!(plain.digest, expiring.digest);
}
