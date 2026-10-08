//! Calldata against viem's `encodeFunctionData` (`viem_evm.json`), selectors
//! against `toFunctionSelector`, and answers against USDC on HyperEVM mainnet.

use serde_json::Value;

use super::*;
use crate::{abi, fixtures};

#[test]
fn calldata_is_what_viem_encodes() {
    let viem = fixtures::viem();
    for case in viem["erc20"].as_array().unwrap() {
        let args = case["args"].as_array().unwrap();
        let amount = |value: &Value| fixtures::text(value).parse::<U256>().unwrap();
        let built = match fixtures::text(&case["function"]) {
            "transfer" => transfer(fixtures::address(fixtures::text(&args[0])), amount(&args[1])),
            "balanceOf" => balance_of(fixtures::address(fixtures::text(&args[0]))),
            "approve" => approve(fixtures::address(fixtures::text(&args[0])), amount(&args[1])),
            "decimals" => decimals(),
            "symbol" => symbol(),
            other => panic!("{other}"),
        };
        assert_eq!(hl_core::hex::encode_prefixed(&built), fixtures::text(&case["calldata"]), "{}", case["function"]);
    }
    for (selector, signature) in [
        (TRANSFER, "transfer(address,uint256)"),
        (BALANCE_OF, "balanceOf(address)"),
        (APPROVE, "approve(address,uint256)"),
        (DECIMALS, "decimals()"),
        (SYMBOL, "symbol()"),
    ] {
        assert_eq!(selector, abi::selector(signature).unwrap(), "{signature}");
    }
}

#[test]
fn answers_read_as_usdc_on_hyperevm_gave_them() {
    let answer = |nth| {
        let said: Value = serde_json::from_str(&fixtures::answer("eth_call", nth)).unwrap();
        fixtures::bytes(&said["result"])
    };
    assert_eq!(read_decimals(&answer(0)), Ok(6));
    assert_eq!(read_symbol(&answer(1)), Ok("USDC".to_owned()));
    assert_eq!(read_amount(&answer(0)), Ok(U256::from(6u8)), "any uint256 word");
    assert!(read_decimals(&[0xff; 32]).is_err(), "a word wider than uint8");
}
