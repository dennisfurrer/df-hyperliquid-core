//! The recorded frames under `fixtures/ws`. Each manifest says where its frames came from in its
//! `source`, and which were trimmed in its `notes` or its `source`; every other frame is the text
//! as received.
//!
//! - `mainnet.json`: recorded from mainnet on 2026-10-08, subscriptions and info posts only.
//! - `testnet.json`: recorded from testnet the same day, with one refused action post.
//! - `portfolio.json`: one mainnet account's clearinghouse frames, recorded 2026-09-26.
//! - `docs.json`: the post-requests page's sample answer to an order.

use crate::utils::Json;

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/mainnet.json"));
const TESTNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/testnet.json"));
const PORTFOLIO: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/portfolio.json"));
const DOCS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/ws/docs.json"));

fn frames(manifest: &str) -> Vec<(String, String)> {
    let manifest = Json::parse(manifest).unwrap();
    manifest
        .get("frames")
        .and_then(Json::as_object)
        .unwrap()
        .iter()
        .map(|(name, text)| (name.clone(), text.as_str().unwrap().to_owned()))
        .collect()
}

fn named(manifest: &str, name: &str) -> String {
    frames(manifest)
        .into_iter()
        .find(|(each, _)| each == name)
        .map(|(_, text)| text)
        .unwrap_or_else(|| panic!("no frame named {name}"))
}

pub fn mainnet(name: &str) -> String {
    named(MAINNET, name)
}

pub fn testnet(name: &str) -> String {
    named(TESTNET, name)
}

pub fn portfolio(name: &str) -> String {
    named(PORTFOLIO, name)
}

pub fn docs(name: &str) -> String {
    named(DOCS, name)
}

/// Every recorded frame, from every manifest.
pub fn all() -> Vec<(String, String)> {
    [MAINNET, TESTNET, PORTFOLIO, DOCS].into_iter().flat_map(frames).collect()
}
