//! Requests are checked against what HyperEVM mainnet was sent and accepted
//! (`hyperevm_mainnet.json`, each call's method and params), answers against
//! what it wrote back, verbatim, and the expected values against node's
//! `BigInt` over the same digits.

use crate::client::evm::fixtures;
use crate::client::evm::rpc::eth;
use serde_json::{Value, json};

use super::*;

fn address(text: &str) -> Address {
    fixtures::address(text)
}

/// The body a query sends, read back as JSON, against the call that was captured.
fn same_request<T>(query: &Query<T>, method: &str, nth: usize) {
    let file = fixtures::mainnet();
    let call = file["calls"].as_array().unwrap().iter().filter(|call| call["method"] == method).nth(nth).unwrap();
    let sent: Value = serde_json::from_str(&query.body()).unwrap();
    assert_eq!(sent, json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": call["params"]}), "{method}");
}

#[test]
fn every_request_is_the_one_the_node_accepted() {
    let usdc = address("0xb88339cb7199b77e23db6e890353e22632ba630f");
    let me = address("0x14791697260e4c9a71f18484c9f997b308e59325");
    let send: B256 = "0x1f7c5f72fb847a4ac78c2ca49ed13b031cee23a93d9cd40025623ce973edc768".parse().unwrap();
    same_request(&eth::block_number(), "eth_blockNumber", 0);
    same_request(&eth::chain_id(), "eth_chainId", 0);
    same_request(&eth::transaction(send), "eth_getTransactionByHash", 0);
    same_request(&eth::receipt(send), "eth_getTransactionReceipt", 0);
    same_request(&eth::block(BlockTag::Number(0x2cc1453), false), "eth_getBlockByNumber", 0);
    let raw_actions = LogFilter {
        from_block: Some(BlockTag::Number(0x2cc1440)),
        to_block: Some(BlockTag::Number(0x2cc1460)),
        address: vec![address("0x3333333333333333333333333333333333333333")],
        topics: vec![Some(vec!["0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3".parse().unwrap()])],
        ..LogFilter::default()
    };
    same_request(&eth::logs(&raw_actions), "eth_getLogs", 0);
    same_request(&eth::code(address("0x3333333333333333333333333333333333333333"), BlockTag::Latest), "eth_getCode", 0);
    same_request(&eth::storage_at(usdc, U256::ZERO, BlockTag::Latest), "eth_getStorageAt", 0);
    same_request(
        &eth::balance(address("0x2222222222222222222222222222222222222222"), BlockTag::Latest),
        "eth_getBalance",
        0,
    );
    same_request(
        &eth::transaction_count(address("0x6b9e773128f453f5c2c60935ee2de2cbc5390a24"), BlockTag::Latest),
        "eth_getTransactionCount",
        0,
    );
    same_request(&eth::gas_price(), "eth_gasPrice", 0);
    same_request(&eth::max_priority_fee(), "eth_maxPriorityFeePerGas", 0);
    same_request(&eth::big_block_gas_price(), "eth_bigBlockGasPrice", 0);
    same_request(&eth::using_big_blocks(me), "eth_usingBigBlocks", 0);
    same_request(&eth::fee_history(2, BlockTag::Latest, &[25, 75]), "eth_feeHistory", 0);
    let balance_of =
        crate::utils::hex::decode("0x70a0823100000000000000000000000014791697260e4c9a71f18484c9f997b308e59325")
            .unwrap();
    same_request(
        &eth::estimate_gas(&CallRequest { from: Some(me), ..CallRequest::to(usdc, balance_of) }),
        "eth_estimateGas",
        0,
    );
    // A creation's estimate: no `to`.
    let init = crate::utils::hex::decode("0x600180600b6000396000f300").unwrap();
    same_request(
        &eth::estimate_gas(&CallRequest { from: Some(me), data: init, ..CallRequest::default() }),
        "eth_estimateGas",
        1,
    );
    same_request(&eth::call(&CallRequest::to(usdc, vec![0x31, 0x3c, 0xe5, 0x67]), BlockTag::Latest), "eth_call", 0);
}

#[test]
fn every_answer_reads_as_the_exact_integer_or_bytes_it_carries() {
    let read = |method: &str, nth: usize| fixtures::answer(method, nth);
    assert_eq!(eth::chain_id().answer(&read("eth_chainId", 0)), Ok(999));
    assert_eq!(eth::block_number().answer(&read("eth_blockNumber", 1)), Ok(0x2dc97bf));
    assert_eq!(
        eth::balance(Address::ZERO, BlockTag::Latest).answer(&read("eth_getBalance", 0)).unwrap().to_string(),
        "947499191167351653726310821"
    );
    assert_eq!(
        eth::transaction_count(Address::ZERO, BlockTag::Latest).answer(&read("eth_getTransactionCount", 0)),
        Ok(0x15ffdf)
    );
    assert_eq!(eth::gas_price().answer(&read("eth_gasPrice", 0)), Ok(U256::from(315_902_611u64)));
    assert_eq!(eth::max_priority_fee().answer(&read("eth_maxPriorityFeePerGas", 0)), Ok(U256::ZERO));
    assert_eq!(eth::big_block_gas_price().answer(&read("eth_bigBlockGasPrice", 0)), Ok(U256::from(100_000_000u64)));
    assert_eq!(eth::using_big_blocks(Address::ZERO).answer(&read("eth_usingBigBlocks", 0)), Ok(false));
    assert_eq!(eth::estimate_gas(&CallRequest::default()).answer(&read("eth_estimateGas", 0)), Ok(0x7b2f));
    assert_eq!(eth::estimate_gas(&CallRequest::default()).answer(&read("eth_estimateGas", 1)), Ok(0xd3a1));
    let decimals = eth::call(&CallRequest::default(), BlockTag::Latest).answer(&read("eth_call", 0)).unwrap();
    assert_eq!(decimals.len(), 32);
    assert_eq!(decimals[31], 6);
    let code = eth::code(Address::ZERO, BlockTag::Latest).answer(&read("eth_getCode", 0)).unwrap();
    assert!(code.windows(4).any(|window| window == [0x17, 0x93, 0x8e, 0x13]), "CoreWriter's code holds sendRawAction");
    let slot =
        eth::storage_at(Address::ZERO, U256::ZERO, BlockTag::Latest).answer(&read("eth_getStorageAt", 0)).unwrap();
    assert_eq!(slot.to_address().unwrap().to_hex(), "0xb5653bf26797d3271dabf31c616fa2b7b216a053");
    assert_eq!(eth::receipt(B256::ZERO).answer(&read("eth_getTransactionReceipt", 1)), Ok(None), "not mined");
}

#[test]
fn fee_history_keeps_its_ratios_as_written() {
    let history =
        eth::fee_history(2, BlockTag::Latest, &[25, 75]).answer(&fixtures::answer("eth_feeHistory", 0)).unwrap();
    let said: Value = serde_json::from_str(&fixtures::answer("eth_feeHistory", 0)).unwrap();
    let said = &said["result"];
    assert_eq!(
        history.oldest_block,
        u64::from_str_radix(fixtures::text(&said["oldestBlock"]).trim_start_matches("0x"), 16).unwrap()
    );
    assert_eq!(history.base_fee_per_gas.len(), 3, "one more than the blocks asked: the next block's");
    assert_eq!(history.base_fee_per_gas[0].to_quantity(), fixtures::text(&said["baseFeePerGas"][0]));
    assert_eq!(history.next_base_fee(), Some(fixtures::text(&said["baseFeePerGas"][2]).parse().unwrap()));
    // The raw text, not an f64's reading of it.
    let raw = fixtures::answer("eth_feeHistory", 0);
    for ratio in &history.gas_used_ratio {
        assert!(
            raw.contains(&format!("{},", ratio.as_str())) || raw.contains(&format!("{}]", ratio.as_str())),
            "{ratio}"
        );
    }
    assert_eq!(history.reward.len(), 2);
    assert_eq!(history.reward[0].len(), 2);
}
