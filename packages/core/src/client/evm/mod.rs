//! HyperEVM and the Arbitrum side of the bridge: typed JSON-RPC, ABI, RLP, EIP-1559
//! transactions, HyperCore's read precompiles and CoreWriter actions. Sans-IO;
//! quantities are exact integers ([`U256`]) and amounts convert to [`crate::utils::Dec`].
#![deny(clippy::float_arithmetic)]

pub mod abi;
pub mod chain;
pub mod corewriter;
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
pub use precompiles::{Read, ReadError};
pub use records::{Block, BlockTransactions, FeeHistory, Log, Receipt, Transaction};
pub use rpc::{BlockTag, CallRequest, ErrorKind, LogFilter, NodeError, Query, Revert, Rpc, RpcError, eth};
pub use tx::{AccessItem, Call, Eip1559, Fees, create_address, tx_hash};
pub use u256::{B256, BadAmount, BadQuantity, U256};
pub use words::Names;

#[cfg(test)]
#[path = "tests/fixtures.rs"]
pub(crate) mod fixtures;
