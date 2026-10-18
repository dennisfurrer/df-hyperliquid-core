//! Typed Ethereum JSON-RPC: each method is a [`Query`] that writes its request
//! body and reads its answer, and [`Rpc`] sends one over an
//! [`Http`](crate::client::Http) to HyperEVM
//! ([`Network::evm_rpc_url`](crate::client::Network::evm_rpc_url)) or any
//! Arbitrum node.
//!
//! HyperEVM's node takes `latest` only for state reads, and an `eth_getLogs`
//! of at most 50 blocks and 4 topics.

pub mod block;
pub mod call;
pub mod client;
pub mod error;
pub mod eth;
pub mod filter;
pub mod query;

pub use block::BlockTag;
pub use call::CallRequest;
pub use client::Rpc;
pub use error::{ErrorKind, NodeError, Revert, RpcError, read_revert};
pub use filter::LogFilter;
pub use query::Query;
