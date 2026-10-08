//! The vectors every test module reads, each file naming what made it:
//!
//! - `viem_evm.json`: viem 2.55.4 alone, and live answers read by it;
//! - `hip4_tui_evm.json`: hip4-tui's EVM tests copied as they are;
//! - `hyperevm_mainnet.json`: HyperEVM mainnet's JSON-RPC answers, verbatim.

use hl_core::{Address, hex};
use serde_json::Value;

const VIEM: &str = include_str!("../../../../vectors/viem_evm.json");
const HIP4_TUI: &str = include_str!("../../../../vectors/hip4_tui_evm.json");
const MAINNET: &str = include_str!("../../../../vectors/hyperevm_mainnet.json");

pub fn viem() -> Value {
    serde_json::from_str(VIEM).unwrap()
}

pub fn hip4_tui() -> Value {
    serde_json::from_str(HIP4_TUI).unwrap()
}

pub fn mainnet() -> Value {
    serde_json::from_str(MAINNET).unwrap()
}

/// The verbatim answer to the `nth` captured call of `method`, counted from zero.
pub fn answer(method: &str, nth: usize) -> String {
    let file = mainnet();
    let found = file["calls"].as_array().unwrap().iter().filter(|call| call["method"] == method).nth(nth);
    found.unwrap_or_else(|| panic!("no {method} #{nth}"))["answer"].as_str().unwrap().to_owned()
}

/// The live contract creation: 0 is the transaction, 1 its receipt.
pub fn creation(nth: usize) -> String {
    mainnet()["creation"][nth]["answer"].as_str().unwrap().to_owned()
}

pub fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_else(|| panic!("{value} is not text"))
}

pub fn bytes(value: &Value) -> Vec<u8> {
    hex::decode(text(value)).unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}
