//! [`Rpc`], one chain's JSON-RPC: a URL and the wire to it, each method sent
//! once and its answer read.

use crate::client::evm::records::{Block, FeeHistory, Log, Receipt, Transaction};
use crate::client::evm::rpc::{BlockTag, CallRequest, LogFilter, Query, RpcError, eth};
use crate::client::evm::u256::{B256, U256};
use crate::client::{Http, Network, Request, TransportError};
use crate::utils::Address;

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
            return Err(TransportError::RateLimited { retry_after_ms: response.retry_after_ms() }.into());
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

    /// [`eth::net_version`].
    pub async fn net_version(&self) -> Result<u64, RpcError> {
        self.run(eth::net_version()).await
    }

    /// [`eth::client_version`].
    pub async fn client_version(&self) -> Result<String, RpcError> {
        self.run(eth::client_version()).await
    }

    /// [`eth::syncing`].
    pub async fn syncing(&self) -> Result<bool, RpcError> {
        self.run(eth::syncing()).await
    }

    /// [`eth::block_by_hash`].
    pub async fn block_by_hash(&self, block: B256, full: bool) -> Result<Option<Block>, RpcError> {
        self.run(eth::block_by_hash(block, full)).await
    }

    /// [`eth::block_receipts`].
    pub async fn block_receipts(&self, block: BlockTag) -> Result<Option<Vec<Receipt>>, RpcError> {
        self.run(eth::block_receipts(block)).await
    }

    /// [`eth::block_receipts_by_hash`].
    pub async fn block_receipts_by_hash(&self, block: B256) -> Result<Option<Vec<Receipt>>, RpcError> {
        self.run(eth::block_receipts_by_hash(block)).await
    }

    /// [`eth::block_transaction_count`].
    pub async fn block_transaction_count(&self, block: BlockTag) -> Result<Option<u64>, RpcError> {
        self.run(eth::block_transaction_count(block)).await
    }

    /// [`eth::block_transaction_count_by_hash`].
    pub async fn block_transaction_count_by_hash(&self, block: B256) -> Result<Option<u64>, RpcError> {
        self.run(eth::block_transaction_count_by_hash(block)).await
    }

    /// [`eth::transaction_by_block_and_index`].
    pub async fn transaction_by_block_and_index(
        &self,
        block: BlockTag,
        position: u64,
    ) -> Result<Option<Transaction>, RpcError> {
        self.run(eth::transaction_by_block_and_index(block, position)).await
    }

    /// [`eth::transaction_by_block_hash_and_index`].
    pub async fn transaction_by_block_hash_and_index(
        &self,
        block: B256,
        position: u64,
    ) -> Result<Option<Transaction>, RpcError> {
        self.run(eth::transaction_by_block_hash_and_index(block, position)).await
    }

    /// [`eth::system_transactions`].
    pub async fn system_transactions(&self, block: BlockTag) -> Result<Option<Vec<Transaction>>, RpcError> {
        self.run(eth::system_transactions(block)).await
    }

    /// [`eth::system_transactions_by_hash`].
    pub async fn system_transactions_by_hash(&self, block: B256) -> Result<Option<Vec<Transaction>>, RpcError> {
        self.run(eth::system_transactions_by_hash(block)).await
    }
}
