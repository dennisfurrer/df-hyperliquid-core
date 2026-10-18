//! Every HIP-4 vector on file, rebuilt through the typed builders (never from the JSON itself) and signed on
//! the network each was made for. The files are in `vectors/`, each naming what made it.
//!
//! - `deploy.rs`, `outcome_deploy.json`: deployer vectors (Hyperliquid's Python SDK, an independent builder,
//!   recorded presets), every one built by the deploy constructors from the inputs as typed. Each keyword is
//!   checked against the hint the venue's own templates give a keyword of that name, and each settlement copies
//!   an `outcomeMeta` listing what it settles.
//! - `session.rs`, `outcomes.json`: a first HIP-4 session through an outcome ticket, signed by
//!   `@nktkas/hyperliquid` 0.24.3, on mainnet.
//! - `orders.rs`, `l1.json`: the HIP-4 market orders among the recorded L1 cases, mainnet; and an order naming
//!   testnet's own outcome, from its recorded `outcomeMeta`, signed for testnet.

mod deploy;
mod orders;
mod session;

use df_hyperliquid_core::utils::{Address, Json};

/// Hardhat's published accounts #0 and #1, `outcomes.json`'s owner and agent.
const OWNER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn number(json: &Json, key: &str) -> u64 {
    json.get(key).and_then(Json::as_u64).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn list<'a>(json: &'a Json, key: &str) -> &'a [Json] {
    json.get(key).and_then(Json::as_array).unwrap_or_else(|| panic!("no {key} in {}", json.compact()))
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}
