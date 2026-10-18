//! Every order-family L1 vector on file, rebuilt through the typed builders rather than read back as a map:
//! the same action JSON, msgpack, hash, and signature on the network the case was made for. Where a file
//! records the msgpack it is compared as written; where it does not, the typed action's msgpack is compared
//! with this crate's reading of the recorded JSON. Each file names the implementation that made it:
//!
//! - `l1.rs`, `l1.json`: `@nktkas/hyperliquid` 0.24.3.
//! - `session.rs`, `tpsl.json`, `leverage.json`, `session.json`, `outcomes.json`: trading and outcome sessions
//!   by the same SDK, signed by the session's agent.
//! - `vault.rs`, `vault.json`: the venue's Python SDK 0.19.0, acting for a subaccount.
//! - `nktkas.rs`, `nktkas-0.33.2-trade.json`: order vectors from `@nktkas/hyperliquid` 0.33.2 on both networks,
//!   the Python SDK's signing tests, and hashes.

mod l1;
mod nktkas;
mod session;
mod vault;

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::trade::builder::fee::BuilderFee;
use df_hyperliquid_core::trade::{
    Amount, BuilderSetting, Cloid, Covers, Exit, Market, OrderIntent, Side, Tif, Tpsl, TriggerIntent,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Address, Asset, Dec, Json, hex};

/// The builder the session and 0.33.2 vectors name.
const BUILDER: &str = "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843";

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn id(id: u32) -> Asset {
    Asset::from_id(id)
}

fn cloid(n: u128) -> Cloid {
    Cloid::from_u128((0x7065_7270_u128 << 96) | n)
}

fn flat(builder: &str, fee: u64) -> BuilderSetting {
    BuilderSetting::flat(address(builder), BuilderFee::from_tenths(fee))
}

fn network(case: &Json) -> Network {
    match case.get("isTestnet") {
        Some(Json::Bool(true)) => Network::Testnet,
        Some(Json::Bool(false)) | None => Network::Mainnet,
        other => panic!("isTestnet is {other:?}"),
    }
}

/// The Python SDK writes `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

fn check(name: &str, typed: &dyn L1Action, case: &Json, key: &LocalKey) {
    let recorded = case.get("action").unwrap();
    assert_eq!(typed.wire(), recorded.compact(), "{name}: the action");
    assert_eq!(typed.msgpack(), recorded.to_action().unwrap().msgpack(), "{name}: msgpack of the recorded JSON");
    if let Some(Json::String(packed)) = case.get("msgpack") {
        assert_eq!(hex::encode_prefixed(&typed.msgpack()), *packed, "{name}: msgpack");
    }
    let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
    let vault = match case.get("vaultAddress") {
        Some(Json::String(vault)) => Some(address(vault)),
        _ => None,
    };
    if let Some(Json::String(hash)) = case.get("actionHash") {
        assert_eq!(hex::encode_prefixed(&typed.connection_id(nonce, vault)), *hash, "{name}: hash");
    }
    let Some(expected) = case.get("signature") else { return };
    let network = network(case);
    let signature = key.sign_digest(&typed.digest(nonce, vault, network)).unwrap();
    assert_eq!(signature.r_hex(), word(text(expected, "r")), "{name} on {network}: r");
    assert_eq!(signature.s_hex(), word(text(expected, "s")), "{name} on {network}: s");
    assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{name}: v");
    // the recorded body leaves out an absent vaultAddress; this crate writes it null, which the venue reads alike
    if let Some(Json::String(body)) = case.get("body") {
        let ours = typed.body(nonce, &signature, vault);
        assert_eq!(ours, format!("{},\"vaultAddress\":null}}", &body[..body.len() - 1]), "{name}: body");
    }
}

fn btc() -> Market {
    Market::perp(id(0), 5, 40, true).unwrap()
}

fn market_buy(size: &str, reference: &str, slippage: &str, n: u128) -> OrderIntent {
    OrderIntent::market(Side::Buy, Amount::Size(dec(size)), dec(reference), dec(slippage)).with_cloid(cloid(n))
}

fn limit(side: Side, price: &str, size: &str, tif: Tif) -> OrderIntent {
    OrderIntent::limit(side, Amount::Size(dec(size)), dec(price), tif)
}

fn exit(side: Side, size: &str, tpsl: Tpsl, at: &str, n: Option<u128>) -> TriggerIntent {
    let intent = TriggerIntent::new(side, Covers::Size(Amount::Size(dec(size))), tpsl, Exit::market(dec(at)));
    match n {
        Some(n) => intent.with_cloid(cloid(n)),
        None => intent,
    }
}
