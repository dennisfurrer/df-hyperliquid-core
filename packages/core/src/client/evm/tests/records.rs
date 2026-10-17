//! Receipts, transactions, blocks and logs read from HyperEVM mainnet's own
//! answers (`hyperevm_mainnet.json`), each field against the text the node
//! wrote, and a transaction rebuilt from its record against its hash.

use serde_json::Value;

use super::*;
use crate::client::evm::fixtures;
use crate::client::evm::rpc::{BlockTag, LogFilter, eth};
use crate::client::evm::tx::tx_hash;
use crate::client::evm::u256::U256;

fn result(answer: &str) -> Value {
    serde_json::from_str::<Value>(answer).unwrap()["result"].clone()
}

fn quantity(value: &Value) -> u64 {
    u64::from_str_radix(fixtures::text(value).trim_start_matches("0x"), 16).unwrap()
}

#[test]
fn a_creations_receipt_keeps_the_contract_it_made() {
    let answer = fixtures::creation(1);
    let said = result(&answer);
    let receipt = eth::receipt(B256::ZERO).answer(&answer).unwrap().unwrap();
    assert_eq!(receipt.to, None);
    assert_eq!(receipt.contract_address.map(|address| address.to_hex()).as_deref(), said["contractAddress"].as_str());
    assert!(receipt.succeeded());
    assert_eq!(receipt.gas_used, quantity(&said["gasUsed"]));
    assert_eq!(receipt.cumulative_gas_used, quantity(&said["cumulativeGasUsed"]));
    assert_eq!(receipt.block_number, Some(quantity(&said["blockNumber"])));
    assert_eq!(receipt.effective_gas_price, Some(fixtures::text(&said["effectiveGasPrice"]).parse().unwrap()));
    assert_eq!(receipt.kind, Some(2));
    assert!(receipt.logs.is_empty());
}

#[test]
fn a_calls_receipt_keeps_its_logs_in_order() {
    let answer = fixtures::answer("eth_getTransactionReceipt", 0);
    let said = result(&answer);
    let receipt = eth::receipt(B256::ZERO).answer(&answer).unwrap().unwrap();
    assert_eq!(receipt.contract_address, None, "a call makes no contract");
    assert_eq!(receipt.to.map(|to| to.to_hex()).as_deref(), said["to"].as_str());
    let logs = said["logs"].as_array().unwrap();
    assert_eq!(receipt.logs.len(), logs.len());
    for (log, raw) in receipt.logs.iter().zip(logs) {
        assert_eq!(log.address.to_hex(), fixtures::text(&raw["address"]));
        let topics: Vec<String> = log.topics.iter().map(B256::to_hex).collect();
        let raw_topics: Vec<&str> = raw["topics"].as_array().unwrap().iter().map(fixtures::text).collect();
        assert_eq!(topics, raw_topics);
        assert_eq!(crate::utils::hex::encode_prefixed(&log.data), fixtures::text(&raw["data"]));
        assert_eq!(log.log_index, Some(quantity(&raw["logIndex"])));
        assert_eq!(log.transaction_hash, Some(receipt.transaction_hash));
        assert!(!log.removed);
    }
}

#[test]
fn a_transaction_record_rebuilds_into_the_transaction_it_hashes_from() {
    for answer in [fixtures::answer("eth_getTransactionByHash", 0), fixtures::creation(0)] {
        let said = result(&answer);
        let record = eth::transaction(B256::ZERO).answer(&answer).unwrap().unwrap();
        assert_eq!(record.hash.to_hex(), fixtures::text(&said["hash"]));
        assert_eq!(record.from.to_hex(), fixtures::text(&said["from"]));
        assert_eq!(record.nonce, quantity(&said["nonce"]));
        assert_eq!(record.gas, quantity(&said["gas"]));
        assert_eq!(crate::utils::hex::encode_prefixed(&record.input), fixtures::text(&said["input"]));
        assert_eq!(record.kind, 2);
        let rebuilt = record.to_eip1559().unwrap();
        let signature = record.signature().unwrap();
        assert_eq!(tx_hash(&rebuilt.signed(&signature).unwrap()), record.hash);
        assert_eq!(rebuilt.signer(&signature).unwrap(), record.from);
    }
}

#[test]
fn a_block_reads_its_header_and_its_transaction_hashes() {
    let answer = fixtures::answer("eth_getBlockByNumber", 0);
    let said = result(&answer);
    let block = eth::block(BlockTag::Latest, false).answer(&answer).unwrap().unwrap();
    assert_eq!(block.number, quantity(&said["number"]));
    assert_eq!(block.hash.to_hex(), fixtures::text(&said["hash"]));
    assert_eq!(block.parent_hash.to_hex(), fixtures::text(&said["parentHash"]));
    assert_eq!(block.timestamp, quantity(&said["timestamp"]));
    assert_eq!(block.gas_limit, 3_000_000, "a small block's limit, as the docs give it");
    assert_eq!(block.gas_used, quantity(&said["gasUsed"]));
    assert_eq!(block.base_fee_per_gas, Some(U256::from(100_000_000u64)));
    let BlockTransactions::Hashes(hashes) = &block.transactions else { panic!("hashes were asked for") };
    let raw: Vec<&str> = said["transactions"].as_array().unwrap().iter().map(fixtures::text).collect();
    assert_eq!(hashes.iter().map(B256::to_hex).collect::<Vec<_>>(), raw);
}

#[test]
fn a_full_block_reads_its_transactions_whole() {
    let tx = result(&fixtures::answer("eth_getTransactionByHash", 0));
    let mut block = result(&fixtures::answer("eth_getBlockByNumber", 0));
    block["transactions"] = Value::Array(vec![tx]);
    let answer = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": block}).to_string();
    let block = eth::block(BlockTag::Latest, true).answer(&answer).unwrap().unwrap();
    let BlockTransactions::Full(transactions) = &block.transactions else { panic!("whole transactions") };
    assert_eq!(transactions[0].nonce, 0x1108e);
}

#[test]
fn a_window_of_logs_reads_every_log() {
    let answer = fixtures::answer("eth_getLogs", 1);
    let said = result(&answer);
    let logs = eth::logs(&LogFilter::default()).answer(&answer).unwrap();
    assert_eq!(logs.len(), 12);
    for (log, raw) in logs.iter().zip(said.as_array().unwrap()) {
        assert_eq!(log.block_number, Some(quantity(&raw["blockNumber"])));
        assert_eq!(log.transaction_index, Some(quantity(&raw["transactionIndex"])));
        assert_eq!(log.block_hash.map(|hash| hash.to_hex()).as_deref(), raw["blockHash"].as_str());
    }
}
