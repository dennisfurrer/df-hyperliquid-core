//! The vectors the EVM tests read: viem 2.55.4's encodings and live answers
//! read by it (`viem_evm.json`), more cases built with viem and live CoreWriter
//! logs (`evm_cases.json`), and HyperEVM mainnet's JSON-RPC answers verbatim
//! (`hyperevm_mainnet.json`).

use crate::utils::{Address, hex};
use serde_json::Value;

const VIEM: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/viem_evm.json"));
const CASES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/evm_cases.json"));
const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/hyperevm_mainnet.json"));

pub fn viem() -> Value {
    serde_json::from_str(VIEM).unwrap()
}

pub fn cases() -> Value {
    serde_json::from_str(CASES).unwrap()
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
