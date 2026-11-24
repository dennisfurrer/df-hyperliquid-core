//! A typed [`Read`]: where the call goes, its calldata and how its answer reads,
//! and the [`Rpc`] method that makes one.

use crate::client::Http;
use crate::client::evm::abi::AbiError;
use crate::client::evm::precompiles::table::address;
use crate::client::evm::rpc::{BlockTag, CallRequest, Rpc, RpcError};
use crate::client::evm::u256::{B256, U256};
use crate::utils::Address;

/// A typed read: where the call goes, its calldata, and how its answer reads.
#[derive(Debug, Clone)]
pub struct Read<T> {
    /// The precompile's address, `0x...0800` through `0x...0814`.
    pub at: Address,
    /// The calldata: the ABI-encoded arguments with no selector.
    pub data: Vec<u8>,
    decode: fn(&[u8]) -> Result<T, AbiError>,
}

impl<T> Read<T> {
    /// The `eth_call` that makes the read.
    #[must_use]
    pub fn request(&self) -> CallRequest {
        CallRequest::to(self.at, self.data.clone())
    }

    /// The answer's bytes, read against the declared struct.
    pub fn decode(&self, answer: &[u8]) -> Result<T, AbiError> {
        (self.decode)(answer)
    }
}

/// Why a precompile read failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    #[error(transparent)]
    /// The `eth_call` failed.
    Rpc(#[from] RpcError),
    #[error("the precompile's answer does not fit its struct: {0}")]
    /// The answer does not decode as the precompile's declared return struct.
    Abi(#[from] AbiError),
}

impl<H: Http> Rpc<H> {
    /// One read precompile, at the latest block: the only one HyperEVM serves.
    pub async fn read<T>(&self, read: &Read<T>) -> Result<T, ReadError> {
        let answer = self.call(&read.request(), BlockTag::Latest).await?;
        Ok(read.decode(&answer)?)
    }
}

/// One argument word: every precompile argument is a static type, so the
/// calldata is the words one after another, as `abi.encode` writes them.
pub(super) enum Arg {
    Address(Address),
    Uint(u64),
}

pub(super) fn read<T>(at: u16, args: &[Arg], decode: fn(&[u8]) -> Result<T, AbiError>) -> Read<T> {
    let mut data = Vec::with_capacity(args.len() * 32);
    for arg in args {
        let word = match arg {
            Arg::Address(address) => B256::from_address(*address).0,
            Arg::Uint(value) => U256::from_u64(*value).to_be_bytes(),
        };
        data.extend_from_slice(&word);
    }
    Read { at: address(at), data, decode }
}
