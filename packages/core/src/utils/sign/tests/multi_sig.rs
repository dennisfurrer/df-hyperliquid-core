//! A multi-sig action's shapes: the payload each signer signs, the action the outer signer posts and
//! what its hash leaves out, and the `SendMultiSig` the outer signer signs. The Python SDK's hashes
//! for outcome actions nested this way are in `tests/conformance/hip4/deploy.rs`.

use super::*;
use crate::account::{LocalKey, recover};
use crate::utils::hex;
use crate::utils::sign::eip712::digest_of_wallet_json;
use crate::utils::sign::l1::l1_digest;

/// The account, typed in mixed case, and its outer signer.
const USER: &str = "0xA8D106881abe9a0b6ffd367788d6af91439d37F6";
const OUTER: &str = "0x251fd19df63a83652eea93a6a5e451aa5ce81e4c";
const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_755_530_000_000;

fn noop() -> Map {
    Map::new().put("type", "noop")
}

fn multi_sig(outer: Address) -> MultiSig {
    MultiSig::new(USER.parse().unwrap(), outer, &noop())
}

/// Two signatures, as the Python SDK's multi-sig vector writes them.
fn signatures() -> Vec<Signature> {
    vec![Signature::from_parts([0x11; 32], [0x22; 32], 27), Signature::from_parts([0x33; 32], [0x44; 32], 28)]
}

#[test]
fn each_signer_signs_the_action_nested_with_the_account_and_the_outer_signer() {
    let multi = multi_sig(OUTER.parse().unwrap());
    assert_eq!(
        multi.payload().to_json().compact(),
        r#"["0xa8d106881abe9a0b6ffd367788d6af91439d37f6","0x251fd19df63a83652eea93a6a5e451aa5ce81e4c",{"type":"noop"}]"#,
        "the addresses lower case, the action whole"
    );
    let packed = multi.payload().to_msgpack();
    assert!(hex::encode(&packed).starts_with("93d92a3078"), "an array of three, the first a 42-byte string");
    assert_eq!(multi.payload_connection_id(NONCE, None), connection_id(&packed, NONCE, None));
    assert_ne!(
        multi.payload_connection_id(NONCE, None),
        multi.payload_connection_id(NONCE, Some(OUTER.parse().unwrap())),
        "a vault is hashed in"
    );
    let digest = multi.payload_digest(NONCE, None, Network::Mainnet);
    assert_eq!(digest, l1_digest(&packed, NONCE, None, Network::Mainnet), "signed as any L1 action");
    assert_ne!(digest, multi.payload_digest(NONCE, None, Network::Testnet));
    assert_eq!(multi.clone().signed(signatures()).payload(), multi.payload(), "no signature is signed over");
}

#[test]
fn the_outer_signer_posts_every_signature_and_the_hash_leaves_out_the_type() {
    let multi = multi_sig(OUTER.parse().unwrap()).signed(signatures());
    let word = |byte: &str| format!("0x{}", byte.repeat(32));
    let posted = multi.action(Network::Testnet);
    assert_eq!(
        posted.to_json().compact(),
        format!(
            concat!(
                r#"{{"type":"multiSig","signatureChainId":"0x66eee","signatures":[{{"r":"{}","s":"{}","v":27}},"#,
                r#"{{"r":"{}","s":"{}","v":28}}],"payload":{{"multiSigUser":"0xa8d106881abe9a0b6ffd367788d6af91439d37f6","#,
                r#""outerSigner":"0x251fd19df63a83652eea93a6a5e451aa5ce81e4c","action":{{"type":"noop"}}}}}}"#,
            ),
            word("11"),
            word("22"),
            word("33"),
            word("44"),
        )
    );
    assert_eq!(
        multi.action(Network::Mainnet).get("signatureChainId"),
        Some(&Value::from("0xa4b1")),
        "the network's signature chain"
    );

    let untyped = posted
        .clone()
        .into_pairs()
        .filter(|(key, _)| key != "type")
        .fold(Map::new(), |map, (key, value)| map.put(key, value));
    let hash = multi.action_hash(NONCE, None, Network::Testnet);
    assert_eq!(hash, connection_id(&untyped.to_msgpack(), NONCE, None));
    assert_ne!(hash, connection_id(&posted.to_msgpack(), NONCE, None), "the type is not hashed");
    assert_ne!(hash, multi.action_hash(NONCE, None, Network::Mainnet), "the chain is");
    assert_ne!(
        hash,
        multi_sig(OUTER.parse().unwrap()).action_hash(NONCE, None, Network::Testnet),
        "so is each signature"
    );
}

#[test]
fn the_outer_signer_signs_send_multi_sig_over_the_hash() {
    let key = LocalKey::from_hex(KEY).unwrap();
    let multi = multi_sig(key.address()).signed(signatures());
    let typed = multi.typed(NONCE, None, Network::Mainnet);
    assert_eq!(
        typed.type_signature(),
        "HyperliquidTransaction:SendMultiSig(string hyperliquidChain,bytes32 multiSigActionHash,uint64 nonce)"
    );
    assert_eq!(typed.fields()[1].1, Field::Bytes32(multi.action_hash(NONCE, None, Network::Mainnet)));
    let digest = multi.digest(NONCE, None, Network::Mainnet);
    assert_eq!(digest, typed.digest(&Domain::user_signed(Network::Mainnet)), "under the user-signed domain");
    assert_ne!(digest, multi.digest(NONCE, None, Network::Testnet));
    assert_eq!(digest_of_wallet_json(&multi.wallet_json(NONCE, None, Network::Mainnet)), Ok(digest));

    let signature = key.sign_digest(&digest).unwrap();
    assert_eq!(recover(&digest, &signature), Ok(key.address()));
    let body = multi.body(NONCE, &signature, None, Network::Mainnet);
    assert!(body.starts_with(r#"{"action":{"type":"multiSig","signatureChainId":"0xa4b1","signatures":["#), "{body}");
    assert!(
        body.ends_with(&format!(r#","nonce":1755530000000,"signature":{},"vaultAddress":null}}"#, signature.to_json())),
        "{body}"
    );
}
