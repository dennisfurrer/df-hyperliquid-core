//! Against testnet, run by hand with `just testnet`: every L1 action in `vectors/` is signed for
//! testnet by a new random key that has no account, and posted to `/exchange`.
//!
//! The node reads the action into its own types, hashes it, and recovers the signer from the
//! signature. With no account behind the key nothing can execute, and the answer shows how far
//! the action got:
//!
//! - `User or API Wallet <address> does not exist.` with the key's own address: the node read
//!   every field name and value, and hashed the action to the same bytes this crate signed.
//! - The same answer with any other address: the node hashed different bytes, so a field is out
//!   of order or has a different type than the node's.
//! - `422 Failed to deserialize the JSON body`: a field name, a tag or a value the node does not
//!   know.
//! - `Must deposit before performing actions. User: <address>`: as the first, for actions the
//!   node checks against a deposit before the account.
//!
//! Three actions the docs list are refused by the node with a 422 for every shape:
//! `enableAlignedQuoteToken`, `disableAlignedQuoteToken`, and `CValidatorAction` `register` with
//! an IPv6 `node_ip`. The test expects that refusal, and fails once the node accepts them.
//!
//! The conformance suite holds every typed constructor to these recorded actions byte for byte,
//! so a pass here covers the constructors too. Units (wei, basis points) are not checked here:
//! a wrong amount still parses and hashes.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Transport;
use df_hyperliquid_core::client::native::{Native, clock};
use df_hyperliquid_core::utils::Json;
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::{Client, Network};

/// Every vector file, by name.
const FILES: [(&str, &str); 13] = [
    ("deploy_actions", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/deploy_actions.json"))),
    ("l1", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"))),
    ("leverage", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/leverage.json"))),
    ("nktkas-0.33.2-trade", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2-trade.json"))),
    ("nktkas-0.33.2", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2.json"))),
    ("nktkas-0.33.3-parity", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.3-parity.json"))),
    ("outcome_deploy", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcome_deploy.json"))),
    ("outcomes", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcomes.json"))),
    ("send_asset", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/send_asset.json"))),
    ("session", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/session.json"))),
    ("tpsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/tpsl.json"))),
    ("validator_actions", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/validator_actions.json"))),
    ("vault", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/vault.json"))),
];

/// A key nobody has used: 32 bytes from the operating system.
fn new_key() -> LocalKey {
    let mut secret = [0u8; 32];
    std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom").unwrap(), &mut secret).unwrap();
    LocalKey::from_bytes(&secret).unwrap()
}

/// Every object under an `action` key that has a `type` and is not user-signed, in file order.
fn l1_actions(json: &Json, out: &mut Vec<Json>) {
    match json {
        Json::Object(entries) => {
            for (key, value) in entries {
                if key == "action" && is_l1(value) {
                    out.push(value.clone());
                }
                // A `reference` entry is the second implementation's output where it differs
                // from this crate's, kept to show the difference; it is not what the crate sends.
                if key != "reference" {
                    l1_actions(value, out);
                }
            }
        }
        Json::Array(items) => items.iter().for_each(|item| l1_actions(item, out)),
        _ => {}
    }
}

fn is_l1(action: &Json) -> bool {
    let kind = action.get("type").and_then(Json::as_str);
    // `dummy` is the Python SDK's hashing test input, not an action.
    kind.is_some_and(|kind| kind != "dummy")
        && action.get("signatureChainId").is_none()
        && action.get("hyperliquidChain").is_none()
}

/// Actions the docs list that the node refuses with a 422 for every shape, as of 2026-11-24. The
/// test fails if one is ever accepted, so the crate can drop its deprecation.
fn refused_by_the_node(wire: &str) -> bool {
    ["\"enableAlignedQuoteToken\"", "\"disableAlignedQuoteToken\"", "\"Ip\":\"2001:db8::1\""]
        .iter()
        .any(|part| wire.contains(part))
}

/// `action` with a top-level `nonce` (`agentSendAsset`) set to the request's nonce, which the
/// node requires them to match.
fn at_nonce(action: Json, nonce: u64) -> Json {
    match action {
        Json::Object(entries) => Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| match key.as_str() {
                    "nonce" => (key, Json::Number(nonce.to_string())),
                    _ => (key, value),
                })
                .collect(),
        ),
        other => other,
    }
}

/// The action's `type`, and for `spotDeploy` and `perpDeploy` the variant key after it.
fn label(action: &Json) -> String {
    let kind = action.get("type").and_then(Json::as_str).unwrap_or("?");
    match action {
        Json::Object(entries) if kind == "spotDeploy" || kind == "perpDeploy" => {
            entries.iter().find(|(key, _)| key != "type").map_or(kind.to_owned(), |(key, _)| format!("{kind}.{key}"))
        }
        _ => kind.to_owned(),
    }
}

#[tokio::test]
#[ignore = "posts to testnet; run with `just testnet`"]
async fn testnet_reads_and_hashes_every_l1_action_as_signed() {
    let key = new_key();
    let me = key.address().to_string().to_lowercase();
    let client = Client::reader(Native::new(), Network::Testnet, clock);
    let mut seen = Vec::new();
    let mut failed = Vec::new();
    for (file, text) in FILES {
        let mut found = Vec::new();
        l1_actions(&Json::parse(text).unwrap(), &mut found);
        for action in found {
            let wire = action.compact();
            if seen.contains(&wire) {
                continue;
            }
            seen.push(wire.clone());
            let nonce = clock();
            let map = at_nonce(action.clone(), nonce).to_action().unwrap();
            let signature = key.sign_digest(&map.digest(nonce, None, Network::Testnet)).unwrap();
            let answer = match client.post("/exchange", map.body(nonce, &signature, None)).await {
                Ok(body) => body,
                Err(error) => format!("{error:?}"),
            };
            let lower = answer.to_lowercase();
            let pass = match refused_by_the_node(&wire) {
                true => lower.contains("status: 422") && lower.contains("failed to deserialize"),
                false => {
                    lower.contains(&format!("user or api wallet {me} does not exist"))
                        || lower.contains(&format!("must deposit before performing actions. user: {me}"))
                }
            };
            let note = if refused_by_the_node(&wire) { " (refused by the node, as expected)" } else { "" };
            println!("{} {file} {}{note}", if pass { "ok  " } else { "FAIL" }, label(&action));
            if !pass {
                failed.push(format!("{file} {wire}\n  {answer}"));
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    }
    println!("{} actions, {} failed", seen.len(), failed.len());
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
