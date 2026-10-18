//! Test helpers: each live precompile call as HyperEVM mainnet answered it
//! (`hyperevm_mainnet.json`), viem's reading of the same bytes (`viem_evm.json`),
//! and the accounts the calls were made for.

use serde_json::Value;

use crate::client::evm::fixtures;
use crate::client::evm::precompiles::Read;

/// Each live call: its `to`, its calldata, and the bytes it answered.
pub(crate) fn live(name: &str) -> (String, String, Vec<u8>) {
    let file = fixtures::mainnet();
    let call = file["precompiles"].as_array().unwrap().iter().find(|call| call["name"] == name).unwrap();
    let answer: Value = serde_json::from_str(fixtures::text(&call["answer"])).unwrap();
    let bytes = answer.get("result").map(fixtures::bytes).unwrap_or_default();
    (fixtures::text(&call["params"][0]["to"]).to_owned(), fixtures::text(&call["params"][0]["data"]).to_owned(), bytes)
}

/// viem's reading of the same answer.
pub(crate) fn viem(name: &str) -> Value {
    let file = fixtures::viem();
    let found = file["precompileAnswers"].as_array().unwrap().iter().find(|one| one["name"] == name).unwrap();
    found["decoded"].clone()
}

pub(crate) fn s(value: &Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        other => fixtures::text(other).to_lowercase(),
    }
}

/// The read's address and calldata are the ones that were sent, and its answer decodes.
pub(crate) fn sent<T>(read: &Read<T>, name: &str) -> T {
    let (to, data, answer) = live(name);
    assert_eq!(read.at.to_hex(), to, "{name}: address");
    assert_eq!(crate::utils::hex::encode_prefixed(&read.data), data, "{name}: calldata");
    read.decode(&answer).unwrap()
}

pub(crate) const SHORT: &str = "0x31ca8395cf837de08b24da3f660e77761dfb974b";
pub(crate) const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
pub(crate) const FOLLOWER: &str = "0x03e161499870b0a37549b16a5d33e0582fe1255e";
pub(crate) const DELEGATOR: &str = "0x0a3343b600d81cdaac131342652241cd32e14349";
pub(crate) const FUND: &str = "0xfefefefefefefefefefefefefefefefefefefefe";

pub(crate) fn a(text: &str) -> crate::utils::Address {
    fixtures::address(text)
}
