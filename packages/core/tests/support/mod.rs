//! What the conformance, mocked and live suites share: the recordings on disk, a pinned clock,
//! a published test key, and [`venue::Venue`], a fake Hyperliquid that answers from recordings.

// Each suite uses its own part of this.
#![allow(dead_code)]

pub mod venue;

use std::path::PathBuf;

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::utils::Json;

/// 2026-10-08 17:27 UTC, when the websocket recordings were taken.
pub const NOW: u64 = 1_791_480_444_000;

/// A clock pinned at [`NOW`].
pub const fn clock() -> u64 {
    NOW
}

/// Hardhat's account #0: a published test key that holds nothing anywhere.
pub const KEY: &str = "0xac0974bec39a17e36ba4f4b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

pub fn key() -> LocalKey {
    LocalKey::from_hex(KEY).unwrap()
}

/// A file under `fixtures/`: what the venue answered, recorded.
pub fn fixture(path: &str) -> String {
    read(&root().join("fixtures").join(path))
}

/// A file under `vectors/`: what other implementations produce.
pub fn vector(path: &str) -> String {
    read(&root().join("vectors").join(path))
}

/// One entry of a recording that keeps several answers under names, as text.
pub fn entry(path: &str, key: &str) -> String {
    let file = Json::parse(&fixture(path)).unwrap();
    let value = file.get(key).unwrap_or_else(|| panic!("{path} has no {key}"));
    value.as_str().map_or_else(|| value.compact(), ToOwned::to_owned)
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
