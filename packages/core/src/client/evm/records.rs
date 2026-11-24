//! Receipts, transactions, blocks, logs and fee history as JSON-RPC answers
//! them. Quantities are exact integers; a block's gas used ratio, the one
//! float a node writes, is kept as the digits it wrote.

use crate::account::Signature;
use crate::client::evm::tx::{AccessItem, EIP1559, Eip1559};
use crate::client::evm::u256::{B256, U256};
use crate::utils::{Address, Figure};
use serde::{Deserialize, Deserializer, de};

/// What `eth_getTransactionReceipt` answers once a transaction is mined.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// The hash of the transaction this receipt is for (`transactionHash`).
    pub transaction_hash: B256,
    /// Its position in the block (`transactionIndex`); `None` when the node leaves it out.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    /// The hash of the block it was mined in (`blockHash`); `None` when the node leaves it out.
    #[serde(default)]
    pub block_hash: Option<B256>,
    /// The number of the block it was mined in (`blockNumber`); `None` when the node leaves it out.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    /// The sender.
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    /// The contract a creation made; `None` for a call.
    #[serde(default)]
    pub contract_address: Option<Address>,
    /// Gas this transaction used (`gasUsed`).
    #[serde(deserialize_with = "quantity")]
    pub gas_used: u64,
    /// Gas used in the block up to and including this transaction (`cumulativeGasUsed`).
    #[serde(deserialize_with = "quantity")]
    pub cumulative_gas_used: u64,
    /// The price paid per unit of gas, in wei (`effectiveGasPrice`); `None` when the node leaves it out.
    #[serde(default)]
    pub effective_gas_price: Option<U256>,
    /// `true` when it succeeded, `false` when it reverted.
    #[serde(default, deserialize_with = "opt_status")]
    pub status: Option<bool>,
    /// The logs it emitted, in order.
    pub logs: Vec<Log>,
    /// The transaction type: 2 for EIP-1559.
    #[serde(default, rename = "type", deserialize_with = "opt_byte")]
    pub kind: Option<u8>,
}

impl Receipt {
    /// Mined and not reverted.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.status == Some(true)
    }
}

/// What `eth_getTransactionByHash` answers, mined or still pending.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// The transaction hash: `keccak256` of its signed bytes.
    pub hash: B256,
    /// 2 for EIP-1559; 0 for legacy.
    #[serde(default, rename = "type", deserialize_with = "byte")]
    pub kind: u8,
    /// The chain it is signed for (`chainId`); `None` for a legacy transaction without one.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub chain_id: Option<u64>,
    /// The sender's nonce.
    #[serde(deserialize_with = "quantity")]
    pub nonce: u64,
    /// The hash of the block it was mined in (`blockHash`); `None` while pending.
    #[serde(default)]
    pub block_hash: Option<B256>,
    /// The number of the block it was mined in (`blockNumber`); `None` while pending.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    /// Its position in the block (`transactionIndex`); `None` while pending.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    /// The sender.
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    /// Wei of the chain's coin sent with it.
    pub value: U256,
    /// The gas limit.
    #[serde(deserialize_with = "quantity")]
    pub gas: u64,
    /// Wei per unit of gas (`gasPrice`): the legacy price, or for type 2 what the node reports as paid.
    #[serde(default)]
    pub gas_price: Option<U256>,
    /// The fee cap in wei per unit of gas (`maxFeePerGas`); `None` for a legacy transaction.
    #[serde(default)]
    pub max_fee_per_gas: Option<U256>,
    /// The tip cap in wei per unit of gas (`maxPriorityFeePerGas`); `None` for a legacy transaction.
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<U256>,
    /// The contracts and storage slots it declares (`accessList`); empty when the node leaves it out.
    #[serde(default)]
    pub access_list: Vec<AccessItem>,
    /// The calldata, or the init code of a creation.
    #[serde(deserialize_with = "bytes")]
    pub input: Vec<u8>,
    /// The signature's `r`; `None` when the node leaves it out.
    #[serde(default)]
    pub r: Option<U256>,
    /// The signature's `s`; `None` when the node leaves it out.
    #[serde(default)]
    pub s: Option<U256>,
    /// The signature's parity, 0 or 1 (`yParity`); `None` when the node leaves it out.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub y_parity: Option<u64>,
}

impl Transaction {
    /// The type 2 transaction this record is of: rebuilt, it hashes to
    /// [`Transaction::hash`] once signed with [`Transaction::signature`].
    #[must_use]
    pub fn to_eip1559(&self) -> Option<Eip1559> {
        if self.kind != EIP1559 {
            return None;
        }
        Some(Eip1559 {
            chain_id: self.chain_id?,
            nonce: self.nonce,
            max_priority_fee: self.max_priority_fee_per_gas?,
            max_fee: self.max_fee_per_gas?,
            gas: self.gas,
            to: self.to,
            value: self.value,
            data: self.input.clone(),
            access_list: self.access_list.clone(),
        })
    }

    /// The signature the record carries, `v` as 27 or 28.
    #[must_use]
    pub fn signature(&self) -> Option<Signature> {
        let parity = u8::try_from(self.y_parity?).ok().filter(|parity| *parity <= 1)?;
        Some(Signature::from_parts(self.r?.to_be_bytes(), self.s?.to_be_bytes(), 27 + parity))
    }
}

/// What `eth_getBlockByNumber` answers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// The block number.
    #[serde(deserialize_with = "quantity")]
    pub number: u64,
    /// The block hash.
    pub hash: B256,
    /// The hash of the block before it (`parentHash`).
    pub parent_hash: B256,
    /// Unix seconds.
    #[serde(deserialize_with = "quantity")]
    pub timestamp: u64,
    #[serde(deserialize_with = "quantity")]
    /// The most gas the block can hold (`gasLimit`).
    pub gas_limit: u64,
    #[serde(deserialize_with = "quantity")]
    /// Gas its transactions used (`gasUsed`).
    pub gas_used: u64,
    #[serde(default)]
    /// The base fee in wei per unit of gas (`baseFeePerGas`); `None` before EIP-1559.
    pub base_fee_per_gas: Option<U256>,
    /// The address that produced the block.
    pub miner: Address,
    /// Its transactions, as hashes or whole records.
    pub transactions: BlockTransactions,
}

/// A block's transactions: their hashes, or the whole records when asked for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum BlockTransactions {
    /// Only the hashes, when the block is asked for without full transactions.
    Hashes(Vec<B256>),
    /// The whole records, when the block is asked for with full transactions.
    Full(Vec<Transaction>),
}

/// One log, from a receipt or `eth_getLogs`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    /// The contract that emitted it.
    pub address: Address,
    /// The indexed topics; the first is the event signature hash unless the event is anonymous.
    pub topics: Vec<B256>,
    /// The non-indexed data, ABI encoded.
    #[serde(deserialize_with = "bytes")]
    pub data: Vec<u8>,
    /// The number of the block it is in (`blockNumber`); `None` while pending.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    /// The hash of the block it is in (`blockHash`); `None` while pending.
    #[serde(default)]
    pub block_hash: Option<B256>,
    /// The hash of the transaction that emitted it (`transactionHash`); `None` while pending.
    #[serde(default)]
    pub transaction_hash: Option<B256>,
    /// That transaction's position in the block (`transactionIndex`); `None` while pending.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    /// Its position among the block's logs (`logIndex`); `None` while pending.
    #[serde(default, deserialize_with = "opt_quantity")]
    pub log_index: Option<u64>,
    /// Dropped by a reorganisation.
    #[serde(default)]
    pub removed: bool,
}

/// What `eth_feeHistory` answers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeHistory {
    /// The number of the first block in the answer (`oldestBlock`).
    #[serde(deserialize_with = "quantity")]
    pub oldest_block: u64,
    /// One per block asked for, and one more: the next block's.
    pub base_fee_per_gas: Vec<U256>,
    /// Each block's gas used over its limit, as the node wrote it.
    pub gas_used_ratio: Vec<Figure>,
    /// Per block, the tip at each percentile asked for.
    #[serde(default)]
    pub reward: Vec<Vec<U256>>,
}

impl FeeHistory {
    /// The base fee of the block after the newest one asked for.
    #[must_use]
    pub fn next_base_fee(&self) -> Option<U256> {
        self.base_fee_per_gas.last().copied()
    }
}

/// A quantity that fits a `u64`: block numbers, nonces, gas.
pub(crate) fn quantity<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = U256::deserialize(deserializer)?;
    value.to_u64().ok_or_else(|| de::Error::custom(format!("{value} does not fit 64 bits")))
}

fn opt_quantity<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    match Option::<U256>::deserialize(deserializer)? {
        Some(value) => {
            value.to_u64().map(Some).ok_or_else(|| de::Error::custom(format!("{value} does not fit 64 bits")))
        }
        None => Ok(None),
    }
}

fn byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let value = quantity(deserializer)?;
    u8::try_from(value).map_err(|_| de::Error::custom(format!("{value} is not a transaction type")))
}

fn opt_byte<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u8>, D::Error> {
    match opt_quantity(deserializer)? {
        Some(value) => {
            u8::try_from(value).map(Some).map_err(|_| de::Error::custom(format!("{value} is not a transaction type")))
        }
        None => Ok(None),
    }
}

fn opt_status<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    match opt_quantity(deserializer)? {
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        Some(other) => Err(de::Error::custom(format!("{other} is not a receipt status"))),
        None => Ok(None),
    }
}

/// `0x` and hex bytes.
pub(crate) fn bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
    let digits = text.strip_prefix("0x").ok_or_else(|| de::Error::custom(format!("`{text}` is not 0x and hex")))?;
    crate::utils::hex::decode(digits).map_err(de::Error::custom)
}

#[cfg(test)]
#[path = "tests/records.rs"]
mod tests;
