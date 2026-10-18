//! Every method, as a [`Query`]: `eth_` and HyperEVM's own, each with its
//! params and how its answer reads.

use crate::client::evm::records::{Block, FeeHistory, Log, Receipt, Transaction};
use crate::client::evm::rpc::query::{address, data, json, quantity, small, tag};
use crate::client::evm::rpc::{BlockTag, CallRequest, LogFilter, Query};
use crate::client::evm::u256::{B256, U256};
use crate::utils::{Address, Json, hex};

fn query<T>(method: &'static str, params: Vec<Json>, read: fn(&str) -> Result<T, String>) -> Query<T> {
    Query { method, params, read }
}

/// `eth_chainId`.
#[must_use]
pub fn chain_id() -> Query<u64> {
    query("eth_chainId", Vec::new(), small)
}

/// `eth_blockNumber`.
#[must_use]
pub fn block_number() -> Query<u64> {
    query("eth_blockNumber", Vec::new(), small)
}

/// `eth_getBalance`: wei of the chain's coin.
#[must_use]
pub fn balance(owner: Address, block: BlockTag) -> Query<U256> {
    query("eth_getBalance", vec![address(owner), tag(block)], json)
}

/// `eth_getTransactionCount`: the next nonce, counting the pool at `Pending`.
#[must_use]
pub fn transaction_count(owner: Address, block: BlockTag) -> Query<u64> {
    query("eth_getTransactionCount", vec![address(owner), tag(block)], small)
}

/// `eth_gasPrice`: on HyperEVM, the next small block's base fee.
#[must_use]
pub fn gas_price() -> Query<U256> {
    query("eth_gasPrice", Vec::new(), json)
}

/// `eth_maxPriorityFeePerGas`: on HyperEVM, always zero for now.
#[must_use]
pub fn max_priority_fee() -> Query<U256> {
    query("eth_maxPriorityFeePerGas", Vec::new(), json)
}

/// `eth_feeHistory` over `blocks` blocks up to `newest`, with the tip at
/// each whole percentile in `percentiles`, 0 to 100, rising.
#[must_use]
pub fn fee_history(blocks: u64, newest: BlockTag, percentiles: &[u8]) -> Query<FeeHistory> {
    let percentiles = percentiles.iter().map(|percent| Json::Number(percent.to_string())).collect();
    query("eth_feeHistory", vec![quantity(U256::from_u64(blocks)), tag(newest), Json::Array(percentiles)], json)
}

/// `eth_estimateGas`, a call or (with no `to`) a creation.
#[must_use]
pub fn estimate_gas(call: &CallRequest) -> Query<u64> {
    query("eth_estimateGas", vec![call.json()], small)
}

/// `eth_call`: what the call returns, as bytes.
#[must_use]
pub fn call(call: &CallRequest, block: BlockTag) -> Query<Vec<u8>> {
    query("eth_call", vec![call.json(), tag(block)], data)
}

/// `eth_sendRawTransaction`: the hash the node took it under.
#[must_use]
pub fn send_raw_transaction(raw: &[u8]) -> Query<B256> {
    query("eth_sendRawTransaction", vec![Json::String(hex::encode_prefixed(raw))], json)
}

/// `eth_getTransactionReceipt`: `None` until it is mined.
#[must_use]
pub fn receipt(hash: B256) -> Query<Option<Receipt>> {
    query("eth_getTransactionReceipt", vec![Json::String(hash.to_hex())], json)
}

/// `eth_getTransactionByHash`: `None` for a hash the node does not know.
#[must_use]
pub fn transaction(hash: B256) -> Query<Option<Transaction>> {
    query("eth_getTransactionByHash", vec![Json::String(hash.to_hex())], json)
}

/// `eth_getBlockByNumber`, with whole transactions when `full`.
#[must_use]
pub fn block(block: BlockTag, full: bool) -> Query<Option<Block>> {
    query("eth_getBlockByNumber", vec![tag(block), Json::Bool(full)], json)
}

/// `eth_getCode`: empty for an account that is not a contract.
#[must_use]
pub fn code(at: Address, block: BlockTag) -> Query<Vec<u8>> {
    query("eth_getCode", vec![address(at), tag(block)], data)
}

/// `eth_getStorageAt`: one storage slot's word.
#[must_use]
pub fn storage_at(at: Address, slot: U256, block: BlockTag) -> Query<B256> {
    query("eth_getStorageAt", vec![address(at), quantity(slot), tag(block)], json)
}

/// `eth_getLogs`.
#[must_use]
pub fn logs(filter: &LogFilter) -> Query<Vec<Log>> {
    query("eth_getLogs", vec![filter.json()], json)
}

/// `eth_bigBlockGasPrice`, HyperEVM's: the next big block's base fee.
#[must_use]
pub fn big_block_gas_price() -> Query<U256> {
    query("eth_bigBlockGasPrice", Vec::new(), json)
}

/// `eth_usingBigBlocks`, HyperEVM's: whether the address's transactions
/// go to the big blocks (`evmUserModify` on HyperCore sets it).
#[must_use]
pub fn using_big_blocks(owner: Address) -> Query<bool> {
    query("eth_usingBigBlocks", vec![address(owner)], json)
}
