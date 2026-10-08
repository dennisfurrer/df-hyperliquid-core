//! HyperEVM, and the Arbitrum side of the bridge: JSON-RPC, ABI, RLP and
//! EIP-1559 transactions, HyperCore's read precompiles, CoreWriter, and moving
//! a token between HyperCore and HyperEVM.
//!
//! Owns:
//! - [`rpc`]: typed JSON-RPC over [`hl_core::Http`], to HyperEVM's RPC on a
//!   [`hl_core::Network`] or any Arbitrum URL; refusals as typed values;
//! - [`abi`]: Solidity's encoding for every type the chain uses, selectors
//!   and event topics from a signature in any spelling;
//! - [`tx`] and [`rlp`]: type 2 transactions, calls and contract creations,
//!   their signing hash, and the raw bytes from a signature; signing with a
//!   key in memory is the `sign` feature;
//! - [`precompiles`]: every read precompile `L1Read.sol` names, typed;
//! - [`corewriter`]: every action id, encoded and decoded, `RawAction` logs
//!   read back, and [`words`] to say them in;
//! - [`link`]: system addresses and the transaction that sends a token home.
//!
//! Sans-IO: no runtime, no clock, no randomness. Quantities are exact
//! integers ([`U256`]); amounts convert to [`hl_core::Dec`] at a named number
//! of decimals. HyperEVM is chain 999 on mainnet and 998 on testnet.
#![deny(clippy::float_arithmetic)]

pub mod abi;
pub mod chain;
pub mod corewriter;
pub mod erc20;
pub mod link;
pub mod precompiles;
pub mod records;
pub mod rlp;
pub mod rpc;
pub mod tx;
pub mod u256;
pub mod words;

pub use abi::{AbiError, Function, Kind, Param, Token};
pub use chain::{ARBITRUM_ONE, ARBITRUM_SEPOLIA, Chain, NATIVE_DECIMALS};
pub use corewriter::{Action, BadAction, CORE_WRITER, RawAction};
pub use link::{HYPE_SYSTEM, Linked, system_address};
pub use precompiles::{Read, ReadError};
pub use records::{Block, BlockTransactions, FeeHistory, Log, Receipt, Transaction};
pub use rpc::{BlockTag, CallRequest, ErrorKind, LogFilter, NodeError, Query, Revert, Rpc, RpcError, eth};
pub use tx::{AccessItem, Call, Eip1559, Fees, create_address, tx_hash};
pub use u256::{B256, BadAmount, BadQuantity, U256};
pub use words::Names;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
