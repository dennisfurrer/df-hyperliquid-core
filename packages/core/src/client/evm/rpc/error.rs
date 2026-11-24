//! Why a query failed: a node's JSON-RPC error and what its code says, the
//! reason a call reverted, and [`RpcError`].

use std::fmt;

use crate::client::TransportError;
use crate::client::evm::abi::{self, Token};
use crate::client::evm::u256::U256;
use crate::utils::hex;

/// A JSON-RPC error object, as the node wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    /// The error code; [`NodeError::kind`] says what it means.
    pub code: i64,
    /// The node's message.
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
    /// Any other code.
    Other,
}

impl NodeError {
    /// What the code says. A -32000 to -32099 error whose message starts `execution reverted` counts as a revert.
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

/// Why a JSON-RPC query failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpcError {
    /// No answer, a rate limit, or an HTTP status with no JSON-RPC error in it.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The node answered with a JSON-RPC error object.
    #[error("{method}: the node refused: {error}")]
    Node {
        /// The JSON-RPC method, such as `eth_call`.
        method: &'static str,
        /// The error object the node wrote.
        error: NodeError,
    },
    /// The node answered, but its result did not read as the expected type.
    #[error("{method}: the node's answer could not be read: {detail}")]
    Unreadable {
        /// The JSON-RPC method, such as `eth_call`.
        method: &'static str,
        /// What could not be read, and why.
        detail: String,
    },
}

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
