//! What the chain answers about itself: receipts, transactions, blocks, logs
//! and fee history, read from JSON-RPC's camelCase objects. Quantities are
//! exact integers; the one float a node writes, a block's gas used ratio, is
//! kept as the digits it wrote.

use hl_core::{Address, Figure, Signature};
use serde::{Deserialize, Deserializer, de};

use crate::{
    tx::{AccessItem, EIP1559, Eip1559},
    u256::{B256, U256},
};

/// What `eth_getTransactionReceipt` answers once a transaction is mined.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub transaction_hash: B256,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<B256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    /// The contract a creation made; `None` for a call.
    #[serde(default)]
    pub contract_address: Option<Address>,
    #[serde(deserialize_with = "quantity")]
    pub gas_used: u64,
    #[serde(deserialize_with = "quantity")]
    pub cumulative_gas_used: u64,
    #[serde(default)]
    pub effective_gas_price: Option<U256>,
    /// `true` when it succeeded, `false` when it reverted.
    #[serde(default, deserialize_with = "opt_status")]
    pub status: Option<bool>,
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
    pub hash: B256,
    /// 2 for EIP-1559; 0 for legacy.
    #[serde(default, rename = "type", deserialize_with = "byte")]
    pub kind: u8,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub chain_id: Option<u64>,
    #[serde(deserialize_with = "quantity")]
    pub nonce: u64,
    #[serde(default)]
    pub block_hash: Option<B256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
    pub from: Address,
    /// `None` for a creation.
    #[serde(default)]
    pub to: Option<Address>,
    pub value: U256,
    #[serde(deserialize_with = "quantity")]
    pub gas: u64,
    #[serde(default)]
    pub gas_price: Option<U256>,
    #[serde(default)]
    pub max_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub access_list: Vec<AccessItem>,
    #[serde(deserialize_with = "bytes")]
    pub input: Vec<u8>,
    #[serde(default)]
    pub r: Option<U256>,
    #[serde(default)]
    pub s: Option<U256>,
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
    #[serde(deserialize_with = "quantity")]
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    /// Unix seconds.
    #[serde(deserialize_with = "quantity")]
    pub timestamp: u64,
    #[serde(deserialize_with = "quantity")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "quantity")]
    pub gas_used: u64,
    #[serde(default)]
    pub base_fee_per_gas: Option<U256>,
    pub miner: Address,
    pub transactions: BlockTransactions,
}

/// A block's transactions: their hashes, or the whole records when asked for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum BlockTransactions {
    Hashes(Vec<B256>),
    Full(Vec<Transaction>),
}

/// One log, from a receipt or `eth_getLogs`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub address: Address,
    pub topics: Vec<B256>,
    #[serde(deserialize_with = "bytes")]
    pub data: Vec<u8>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub block_number: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<B256>,
    #[serde(default)]
    pub transaction_hash: Option<B256>,
    #[serde(default, deserialize_with = "opt_quantity")]
    pub transaction_index: Option<u64>,
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
    hl_core::hex::decode(digits).map_err(de::Error::custom)
}

#[cfg(test)]
#[path = "tests/records.rs"]
mod tests;
