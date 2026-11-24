//! Every method, as a [`Query`]: `eth_` and HyperEVM's own, each with its
//! params and how its answer reads.

use crate::client::evm::records::{Block, FeeHistory, Log, Receipt, Transaction};
use crate::client::evm::rpc::query::{
    address, data, decimal, hash, index, json, quantity, small, small_or_null, sync_state, tag,
};
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

/// `net_version`: the network id, which on HyperEVM is the chain id (999 on mainnet).
#[must_use]
pub fn net_version() -> Query<u64> {
    query("net_version", Vec::new(), decimal)
}

/// `web3_clientVersion`: the node's name for itself (`hyperliquid evm Mainnet`).
#[must_use]
pub fn client_version() -> Query<String> {
    query("web3_clientVersion", Vec::new(), json)
}

/// `eth_syncing`: whether the node is still catching up. HyperEVM's node always answers `false`.
#[must_use]
pub fn syncing() -> Query<bool> {
    query("eth_syncing", Vec::new(), sync_state)
}

/// `eth_getBlockByHash`, with whole transactions when `full`: `None` for a hash the node does not
/// know.
#[must_use]
pub fn block_by_hash(block: B256, full: bool) -> Query<Option<Block>> {
    query("eth_getBlockByHash", vec![hash(block), Json::Bool(full)], json)
}

/// `eth_getBlockReceipts`: every receipt of the block, in order, not counting HyperEVM's system
/// transactions. A block past the head is refused.
#[must_use]
pub fn block_receipts(block: BlockTag) -> Query<Option<Vec<Receipt>>> {
    query("eth_getBlockReceipts", vec![tag(block)], json)
}

/// [`block_receipts`] of the block with this hash; `None` for a hash the node does not know.
#[must_use]
pub fn block_receipts_by_hash(block: B256) -> Query<Option<Vec<Receipt>>> {
    query("eth_getBlockReceipts", vec![hash(block)], json)
}

/// `eth_getBlockTransactionCountByNumber`: how many transactions the block holds, not counting
/// HyperEVM's system transactions. A block past the head is refused.
#[must_use]
pub fn block_transaction_count(block: BlockTag) -> Query<Option<u64>> {
    query("eth_getBlockTransactionCountByNumber", vec![tag(block)], small_or_null)
}

/// `eth_getBlockTransactionCountByHash`: [`block_transaction_count`] of the block with this hash;
/// `None` for a hash the node does not know.
#[must_use]
pub fn block_transaction_count_by_hash(block: B256) -> Query<Option<u64>> {
    query("eth_getBlockTransactionCountByHash", vec![hash(block)], small_or_null)
}

/// `eth_getTransactionByBlockNumberAndIndex`: the block's transaction at `position`, counting
/// from 0; `None` past the last.
#[must_use]
pub fn transaction_by_block_and_index(block: BlockTag, position: u64) -> Query<Option<Transaction>> {
    query("eth_getTransactionByBlockNumberAndIndex", vec![tag(block), index(position)], json)
}

/// `eth_getTransactionByBlockHashAndIndex`: [`transaction_by_block_and_index`] in the block with
/// this hash.
#[must_use]
pub fn transaction_by_block_hash_and_index(block: B256, position: u64) -> Query<Option<Transaction>> {
    query("eth_getTransactionByBlockHashAndIndex", vec![hash(block), index(position)], json)
}

/// `eth_getSystemTxsByBlockNumber`, HyperEVM's: the system transactions of the block, the ones
/// HyperCore makes when it sends a token to HyperEVM. Each is a legacy transaction from the token's
/// system address, with no gas price and a zero signature. A block past the head is refused.
#[must_use]
pub fn system_transactions(block: BlockTag) -> Query<Option<Vec<Transaction>>> {
    query("eth_getSystemTxsByBlockNumber", vec![tag(block)], json)
}

/// `eth_getSystemTxsByBlockHash`, HyperEVM's: [`system_transactions`] of the block with this hash;
/// `None` for a hash the node does not know.
#[must_use]
pub fn system_transactions_by_hash(block: B256) -> Query<Option<Vec<Transaction>>> {
    query("eth_getSystemTxsByBlockHash", vec![hash(block)], json)
}
