//! The recorded frames under `fixtures/`. Each manifest says where its frames came from in its
//! `source`, and in `notes` which frames were trimmed; every other frame is the text as received.
//!
//! - `mainnet.json`: recorded from mainnet on 2026-10-08, subscriptions and info posts only.
//! - `testnet.json`: recorded from testnet the same day, with one refused action post.
//! - `hip4-tui.json`: lifted from hip4-tui's `net/src/tests/portfolio.rs` (mainnet, 2026-09-26).
//! - `docs.json`: the post-requests page's sample answer to an order.

use hl_core::Json;

const MAINNET: &str = include_str!("fixtures/mainnet.json");
const TESTNET: &str = include_str!("fixtures/testnet.json");
const HIP4_TUI: &str = include_str!("fixtures/hip4-tui.json");
const DOCS: &str = include_str!("fixtures/docs.json");

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

pub fn hip4_tui(name: &str) -> String {
    named(HIP4_TUI, name)
}

pub fn docs(name: &str) -> String {
    named(DOCS, name)
}

/// Every recorded frame, from every manifest.
pub fn all() -> Vec<(String, String)> {
    [MAINNET, TESTNET, HIP4_TUI, DOCS].into_iter().flat_map(frames).collect()
}
