//! Filters, block tags and call objects as the node takes them.

use crate::client::evm::fixtures::address;
use crate::client::evm::rpc::{CallRequest, eth};
use crate::client::evm::u256::U256;
use serde_json::{Value, json};

use super::*;

#[test]
fn a_log_filter_writes_one_address_bare_and_a_wildcard_as_null() {
    let topic: B256 = "0x8c7f585fb295f7eb1e6aeb8fba61b23a4fe60beda405f0045073b185c74412e3".parse().unwrap();
    let filter = LogFilter {
        address: vec![address("0x3333333333333333333333333333333333333333"), Address::ZERO],
        topics: vec![None, Some(vec![topic, B256::ZERO])],
        block_hash: Some(B256::ZERO),
        ..LogFilter::default()
    };
    let sent: Value = serde_json::from_str(&eth::logs(&filter).body()).unwrap();
    assert_eq!(
        sent["params"][0],
        json!({
            "blockHash": B256::ZERO.to_hex(),
            "address": ["0x3333333333333333333333333333333333333333", Address::ZERO.to_hex()],
            "topics": [null, [topic.to_hex(), B256::ZERO.to_hex()]],
        })
    );
    let tags: Vec<String> = [
        BlockTag::Latest,
        BlockTag::Pending,
        BlockTag::Earliest,
        BlockTag::Safe,
        BlockTag::Finalized,
        BlockTag::Number(0),
    ]
    .iter()
    .map(|tag| tag.to_string())
    .collect();
    assert_eq!(tags, ["latest", "pending", "earliest", "safe", "finalized", "0x0"]);
    let sent: Value = serde_json::from_str(&eth::send_raw_transaction(&[0x02, 0xc0]).body()).unwrap();
    assert_eq!(sent["params"], json!(["0x02c0"]));
    let call =
        CallRequest { gas: Some(21_000), value: Some(U256::from(5u8)), ..CallRequest::to(Address::ZERO, Vec::new()) };
    let sent: Value = serde_json::from_str(&eth::call(&call, BlockTag::Pending).body()).unwrap();
    assert_eq!(sent["params"], json!([{"to": Address::ZERO.to_hex(), "gas": "0x5208", "value": "0x5"}, "pending"]));
}
