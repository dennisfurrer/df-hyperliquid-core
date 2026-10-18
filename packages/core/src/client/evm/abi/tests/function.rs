//! Selectors and topics against viem's `toFunctionSelector` and `toEventSelector`, calldata
//! against `encodeFunctionData`, and live `eth_call` answers.

use serde_json::Value;

use crate::client::evm::abi::support::{hex, token};
use crate::client::evm::fixtures;

use super::*;

#[test]
fn selectors_and_topics_are_computed_from_any_spelling() {
    let viem = fixtures::viem();
    for case in viem["selectors"].as_array().unwrap() {
        let signature = fixtures::text(&case["signature"]);
        assert_eq!(hex(&selector(signature).unwrap()), fixtures::text(&case["selector"]), "{signature}");
    }
    for case in viem["topics"].as_array().unwrap() {
        let signature = fixtures::text(&case["signature"]);
        assert_eq!(event_topic(signature).unwrap().to_hex(), fixtures::text(&case["topic"]), "{signature}");
    }
    // The ABI spec hashes `uint` as `uint256`; viem's toFunctionSelector hashes the shorthand as written.
    assert_eq!(selector("g(uint, int)").unwrap(), selector("g(uint256,int256)").unwrap());
    let transfer = Function::parse("function transfer(address to, uint256 amount) external returns (bool)").unwrap();
    assert_eq!(transfer.signature(), "transfer(address,uint256)");
    assert_eq!(transfer.name, "transfer");
    assert_eq!(transfer.outputs.len(), 1);
    assert!(selector("transfer(address,uint256").is_err());
    assert!(selector("(address)").is_err(), "a function has a name");
}

#[test]
fn a_function_writes_viems_calldata_and_reads_its_answer() {
    let viem = fixtures::viem();
    let functions = [
        "transfer(address to, uint256 amount) returns (bool)",
        "balanceOf(address owner) view returns (uint256)",
        "approve(address spender, uint256 amount) returns (bool)",
        "decimals() view returns (uint8)",
        "symbol() view returns (string)",
    ];
    for case in viem["erc20"].as_array().unwrap() {
        let name = fixtures::text(&case["function"]);
        let function =
            functions.iter().map(|text| Function::parse(text).unwrap()).find(|one| one.name == name).unwrap();
        let args: Vec<Token> = function
            .inputs
            .iter()
            .zip(case["args"].as_array().unwrap())
            .map(|(param, value)| token(&param.kind, value))
            .collect();
        let calldata = function.encode(&args).unwrap();
        assert_eq!(hex(&calldata), fixtures::text(&case["calldata"]), "{name}");
        assert_eq!(function.decode_input(&calldata).unwrap(), args, "{name}");
    }
    // symbol() of USDC on HyperEVM, live.
    let symbol = Function::parse("symbol() returns (string)").unwrap();
    let answer: Value = serde_json::from_str(&fixtures::answer("eth_call", 1)).unwrap();
    let said = symbol.decode_output(&fixtures::bytes(&answer["result"])).unwrap();
    assert_eq!(said, [Token::String("USDC".to_owned())]);
    let decimals = Function::parse("decimals() returns (uint8)").unwrap();
    let answer: Value = serde_json::from_str(&fixtures::answer("eth_call", 0)).unwrap();
    assert_eq!(decimals.decode_output(&fixtures::bytes(&answer["result"])).unwrap()[0].as_u8(), Some(6));
    assert!(decimals.decode_input(&[0xde, 0xad, 0xbe, 0xef]).is_err(), "another function's selector");
}
