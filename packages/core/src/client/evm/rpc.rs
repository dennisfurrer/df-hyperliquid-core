//! Typed Ethereum JSON-RPC: each method is a [`Query`] that writes its request
//! body and reads its answer, and [`Rpc`] sends one over an [`Http`] to HyperEVM
//! ([`Network::evm_rpc_url`]) or any Arbitrum node.
//!
//! HyperEVM's node takes `latest` only for state reads, and an `eth_getLogs`
//! of at most 50 blocks and 4 topics.

use std::fmt;

use crate::client::evm::abi::{self, Token};
use crate::client::evm::records::{Block, FeeHistory, Log, Receipt, Transaction};
use crate::client::evm::tx::Call;
use crate::client::evm::u256::{B256, U256};
use crate::client::{Http, Network, Request, TransportError};
use crate::utils::{Address, Json, hex};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::value::RawValue;

/// Which block a read is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockTag {
    #[default]
    Latest,
    /// Counting what waits in the pool: the next nonce to use.
    Pending,
    Earliest,
    Safe,
    Finalized,
    Number(u64),
}

impl fmt::Display for BlockTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockTag::Latest => f.write_str("latest"),
            BlockTag::Pending => f.write_str("pending"),
            BlockTag::Earliest => f.write_str("earliest"),
            BlockTag::Safe => f.write_str("safe"),
            BlockTag::Finalized => f.write_str("finalized"),
            BlockTag::Number(number) => write!(f, "{number:#x}"),
        }
    }
}

/// The call object `eth_call` and `eth_estimateGas` take. No `to` estimates
/// or runs a contract creation from `data`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallRequest {
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub gas: Option<u64>,
    pub value: Option<U256>,
    pub data: Vec<u8>,
}

impl CallRequest {
    /// `data` sent to a contract.
    #[must_use]
    pub fn to(to: Address, data: Vec<u8>) -> CallRequest {
        CallRequest { to: Some(to), data, ..CallRequest::default() }
    }

    /// What `call` does, as sent by `from`: what a transaction's gas is estimated with.
    #[must_use]
    pub fn of(from: Option<Address>, call: &Call) -> CallRequest {
        CallRequest {
            from,
            to: call.to,
            gas: None,
            value: (!call.value.is_zero()).then_some(call.value),
            data: call.data.clone(),
        }
    }

    fn json(&self) -> Json {
        let mut fields = Vec::new();
        if let Some(from) = self.from {
            fields.push(("from".to_owned(), Json::String(from.to_hex())));
        }
        if let Some(to) = self.to {
            fields.push(("to".to_owned(), Json::String(to.to_hex())));
        }
        if let Some(gas) = self.gas {
            fields.push(("gas".to_owned(), quantity(U256::from_u64(gas))));
        }
        if let Some(value) = self.value {
            fields.push(("value".to_owned(), quantity(value)));
        }
        if !self.data.is_empty() {
            fields.push(("data".to_owned(), Json::String(hex::encode_prefixed(&self.data))));
        }
        Json::Object(fields)
    }
}

/// What `eth_getLogs` matches. One address is written bare and several as a
/// list; in `topics`, `None` matches anything at its place and several
/// topics at one place match any of them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogFilter {
    pub from_block: Option<BlockTag>,
    pub to_block: Option<BlockTag>,
    /// In place of the two blocks: the one block with this hash.
    pub block_hash: Option<B256>,
    pub address: Vec<Address>,
    pub topics: Vec<Option<Vec<B256>>>,
}

impl LogFilter {
    fn json(&self) -> Json {
        let mut fields = Vec::new();
        if let Some(from) = self.from_block {
            fields.push(("fromBlock".to_owned(), Json::String(from.to_string())));
        }
        if let Some(to) = self.to_block {
            fields.push(("toBlock".to_owned(), Json::String(to.to_string())));
        }
        if let Some(hash) = self.block_hash {
            fields.push(("blockHash".to_owned(), Json::String(hash.to_hex())));
        }
        match self.address.as_slice() {
            [] => {}
            [one] => fields.push(("address".to_owned(), Json::String(one.to_hex()))),
            many => fields.push((
                "address".to_owned(),
                Json::Array(many.iter().map(|address| Json::String(address.to_hex())).collect()),
            )),
        }
        if !self.topics.is_empty() {
            let topics = self
                .topics
                .iter()
                .map(|place| match place.as_deref() {
                    None => Json::Null,
                    Some([one]) => Json::String(one.to_hex()),
                    Some(many) => Json::Array(many.iter().map(|topic| Json::String(topic.to_hex())).collect()),
                })
                .collect();
            fields.push(("topics".to_owned(), Json::Array(topics)));
        }
        Json::Object(fields)
    }
}

/// A JSON-RPC error object, as the node wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    pub code: i64,
    pub message: String,
    /// `data`: a string's text, or the JSON as written. A revert's data is hex.
    pub data: Option<String>,
}

/// What a [`NodeError`]'s code says, per JSON-RPC and the Ethereum nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// -32700: the body was not JSON.
    Parse,
    /// -32600.
    InvalidRequest,
    /// -32601: the node does not serve this method.
    MethodNotFound,
    /// -32602.
    InvalidParams,
    /// -32603.
    Internal,
    /// 3, or a server error that says `execution reverted`.
    Reverted,
    /// -32000 to -32099: the node's own, a timeout or a precompile's refusal among them.
    Server,
    Other,
}

impl NodeError {
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self.code {
            -32700 => ErrorKind::Parse,
            -32600 => ErrorKind::InvalidRequest,
            -32601 => ErrorKind::MethodNotFound,
            -32602 => ErrorKind::InvalidParams,
            -32603 => ErrorKind::Internal,
            3 => ErrorKind::Reverted,
            -32099..=-32000 if self.message.starts_with("execution reverted") => ErrorKind::Reverted,
            -32099..=-32000 => ErrorKind::Server,
            _ => ErrorKind::Other,
        }
    }

    /// Why a call reverted, when the node passed the revert data on.
    #[must_use]
    pub fn revert(&self) -> Option<Revert> {
        let data = self.data.as_deref()?.strip_prefix("0x")?;
        hex::decode(data).ok().map(|bytes| read_revert(&bytes))
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)?;
        match &self.data {
            Some(data) => write!(f, ": {data}"),
            None => Ok(()),
        }
    }
}

/// What a revert's data says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revert {
    /// `Error(string)`, from `require` and `revert("...")`.
    Message(String),
    /// `Panic(uint256)`: 0x11 is an overflow, 0x01 a failed assert.
    Panic(U256),
    /// A custom error, or nothing at all: the bytes as they came.
    Custom(Vec<u8>),
}

/// `Error(string)`.
const ERROR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];
/// `Panic(uint256)`.
const PANIC: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];

/// Reads revert data: a message, a panic code, or the bytes of anything else.
#[must_use]
pub fn read_revert(data: &[u8]) -> Revert {
    let custom = || Revert::Custom(data.to_vec());
    let Some((selector, rest)) = data.split_first_chunk::<4>() else {
        return custom();
    };
    let shape = match *selector {
        ERROR => "string",
        PANIC => "uint256",
        _ => return custom(),
    };
    let decoded = abi::params(shape).and_then(|params| abi::decode(&params, rest));
    match decoded.as_deref() {
        Ok([Token::String(text)]) => Revert::Message(text.clone()),
        Ok([Token::Uint(code)]) => Revert::Panic(*code),
        _ => custom(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpcError {
    /// No answer, a rate limit, or an HTTP status with no JSON-RPC error in it.
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error("{method}: the node refused: {error}")]
    Node { method: &'static str, error: NodeError },
    #[error("{method}: the node's answer could not be read: {detail}")]
    Unreadable { method: &'static str, detail: String },
}

/// One JSON-RPC method with its params, and how its `result` is read.
#[derive(Debug, Clone)]
pub struct Query<T> {
    method: &'static str,
    params: Vec<Json>,
    read: fn(&str) -> Result<T, String>,
}

impl<T> Query<T> {
    #[must_use]
    pub const fn method(&self) -> &'static str {
        self.method
    }

    /// `{"jsonrpc":"2.0","id":1,"method":...,"params":[...]}`.
    #[must_use]
    pub fn body(&self) -> String {
        Json::Object(vec![
            ("jsonrpc".to_owned(), Json::String("2.0".to_owned())),
            ("id".to_owned(), Json::Number("1".to_owned())),
            ("method".to_owned(), Json::String(self.method.to_owned())),
            ("params".to_owned(), Json::Array(self.params.clone())),
        ])
        .compact()
    }

    /// The answer's text, read: its `result` as `T`, or its `error`.
    pub fn answer(&self, text: &str) -> Result<T, RpcError> {
        let unreadable = |detail: String| RpcError::Unreadable { method: self.method, detail };
        let envelope: Envelope<'_> = serde_json::from_str(text)
            .map_err(|error| unreadable(format!("{error} in {}", text.chars().take(300).collect::<String>())))?;
        if let Some(error) = envelope.error {
            let data = error
                .data
                .map(|data| serde_json::from_str::<String>(data.get()).unwrap_or_else(|_| data.get().to_owned()));
            return Err(RpcError::Node {
                method: self.method,
                error: NodeError { code: error.code, message: error.message, data },
            });
        }
        let result = envelope.result.ok_or_else(|| unreadable("neither a result nor an error".to_owned()))?;
        (self.read)(result.get()).map_err(|detail| unreadable(format!("{detail} in {}", result.get())))
    }
}

#[derive(Deserialize)]
struct Envelope<'a> {
    #[serde(default, borrow, deserialize_with = "raw")]
    result: Option<&'a RawValue>,
    #[serde(default, borrow)]
    error: Option<ErrorBody<'a>>,
}

#[derive(Deserialize)]
struct ErrorBody<'a> {
    code: i64,
    #[serde(default)]
    message: String,
    #[serde(default, borrow, deserialize_with = "raw")]
    data: Option<&'a RawValue>,
}

/// A value as the text it was written in, `null` included.
fn raw<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<&'de RawValue>, D::Error> {
    <&RawValue>::deserialize(deserializer).map(Some)
}

fn json<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

fn small(text: &str) -> Result<u64, String> {
    let value: U256 = json(text)?;
    value.to_u64().ok_or_else(|| format!("{value} does not fit 64 bits"))
}

fn data(text: &str) -> Result<Vec<u8>, String> {
    let said: String = json(text)?;
    let digits = said.strip_prefix("0x").ok_or_else(|| format!("`{said}` is not 0x and hex"))?;
    hex::decode(digits).map_err(|error| error.to_string())
}

fn quantity(value: U256) -> Json {
    Json::String(value.to_quantity())
}

fn tag(block: BlockTag) -> Json {
    Json::String(block.to_string())
}

fn address(address: Address) -> Json {
    Json::String(address.to_hex())
}

/// Every method, as a [`Query`].
pub mod eth {
    use super::*;

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
}

/// One chain's JSON-RPC: a URL and the wire to it.
#[derive(Debug, Clone)]
pub struct Rpc<H> {
    http: H,
    url: String,
}

impl<H: Http> Rpc<H> {
    /// Any node: an Arbitrum RPC, or HyperEVM through a provider.
    pub fn new(http: H, url: impl Into<String>) -> Self {
        Rpc { http, url: url.into() }
    }

    /// HyperEVM's own RPC on `network`.
    pub fn hyperevm(http: H, network: Network) -> Self {
        Rpc::new(http, network.evm_rpc_url())
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub const fn http(&self) -> &H {
        &self.http
    }

    /// Posts the query once and reads the answer. Nothing is retried.
    pub async fn run<T>(&self, query: Query<T>) -> Result<T, RpcError> {
        let response = self.http.send(Request::post_json(self.url.clone(), query.body())).await?;
        if response.status == 429 {
            let seconds = response.header("retry-after").and_then(|seconds| seconds.trim().parse::<u64>().ok());
            return Err(TransportError::RateLimited {
                retry_after_ms: seconds.map(|seconds| seconds.saturating_mul(1_000)),
            }
            .into());
        }
        let text = response.text();
        match (response.is_success(), query.answer(&text)) {
            (_, Err(refused @ RpcError::Node { .. })) => Err(refused),
            (true, answer) => answer,
            (false, _) => Err(TransportError::Status { status: response.status, body: text }.into()),
        }
    }

    pub async fn chain_id(&self) -> Result<u64, RpcError> {
        self.run(eth::chain_id()).await
    }

    pub async fn block_number(&self) -> Result<u64, RpcError> {
        self.run(eth::block_number()).await
    }

    pub async fn balance(&self, owner: Address, block: BlockTag) -> Result<U256, RpcError> {
        self.run(eth::balance(owner, block)).await
    }

    pub async fn transaction_count(&self, owner: Address, block: BlockTag) -> Result<u64, RpcError> {
        self.run(eth::transaction_count(owner, block)).await
    }

    pub async fn gas_price(&self) -> Result<U256, RpcError> {
        self.run(eth::gas_price()).await
    }

    pub async fn max_priority_fee(&self) -> Result<U256, RpcError> {
        self.run(eth::max_priority_fee()).await
    }

    pub async fn fee_history(&self, blocks: u64, newest: BlockTag, percentiles: &[u8]) -> Result<FeeHistory, RpcError> {
        self.run(eth::fee_history(blocks, newest, percentiles)).await
    }

    pub async fn estimate_gas(&self, call: &CallRequest) -> Result<u64, RpcError> {
        self.run(eth::estimate_gas(call)).await
    }

    pub async fn call(&self, call: &CallRequest, block: BlockTag) -> Result<Vec<u8>, RpcError> {
        self.run(eth::call(call, block)).await
    }

    /// Sent once. With no answer the transaction may still have landed: look
    /// for its receipt by [`crate::client::evm::tx::tx_hash`] before sending it again.
    pub async fn send_raw_transaction(&self, raw: &[u8]) -> Result<B256, RpcError> {
        self.run(eth::send_raw_transaction(raw)).await
    }

    pub async fn receipt(&self, hash: B256) -> Result<Option<Receipt>, RpcError> {
        self.run(eth::receipt(hash)).await
    }

    pub async fn transaction(&self, hash: B256) -> Result<Option<Transaction>, RpcError> {
        self.run(eth::transaction(hash)).await
    }

    pub async fn block(&self, block: BlockTag, full: bool) -> Result<Option<Block>, RpcError> {
        self.run(eth::block(block, full)).await
    }

    pub async fn code(&self, at: Address, block: BlockTag) -> Result<Vec<u8>, RpcError> {
        self.run(eth::code(at, block)).await
    }

    pub async fn storage_at(&self, at: Address, slot: U256, block: BlockTag) -> Result<B256, RpcError> {
        self.run(eth::storage_at(at, slot, block)).await
    }

    pub async fn logs(&self, filter: &LogFilter) -> Result<Vec<Log>, RpcError> {
        self.run(eth::logs(filter)).await
    }

    pub async fn big_block_gas_price(&self) -> Result<U256, RpcError> {
        self.run(eth::big_block_gas_price()).await
    }

    pub async fn using_big_blocks(&self, owner: Address) -> Result<bool, RpcError> {
        self.run(eth::using_big_blocks(owner)).await
    }
}

#[cfg(test)]
#[path = "tests/rpc.rs"]
mod tests;
