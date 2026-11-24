//! [`Query`], one JSON-RPC method with its params: the request body it writes,
//! the envelope its answer comes in, and how its `result` is read.

use crate::client::evm::rpc::{BlockTag, NodeError, RpcError};
use crate::client::evm::u256::{B256, U256};
use crate::utils::{Address, Json, hex};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::value::RawValue;

/// One JSON-RPC method with its params, and how its `result` is read.
#[derive(Debug, Clone)]
pub struct Query<T> {
    pub(super) method: &'static str,
    pub(super) params: Vec<Json>,
    pub(super) read: fn(&str) -> Result<T, String>,
}

impl<T> Query<T> {
    /// The JSON-RPC method name, such as `eth_call`.
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

pub(super) fn json<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

pub(super) fn small(text: &str) -> Result<u64, String> {
    let value: U256 = json(text)?;
    value.to_u64().ok_or_else(|| format!("{value} does not fit 64 bits"))
}

/// A quantity that fits a `u64`, or `null`.
pub(super) fn small_or_null(text: &str) -> Result<Option<u64>, String> {
    match json::<Option<U256>>(text)? {
        Some(value) => value.to_u64().map(Some).ok_or_else(|| format!("{value} does not fit 64 bits")),
        None => Ok(None),
    }
}

/// A decimal string, as `net_version` answers: `"999"`.
pub(super) fn decimal(text: &str) -> Result<u64, String> {
    let said: String = json(text)?;
    said.parse().map_err(|_| format!("`{said}` is not a decimal number"))
}

/// `false`, or an object with the sync's progress, read as `true`.
pub(super) fn sync_state(text: &str) -> Result<bool, String> {
    match json::<serde_json::Value>(text)? {
        serde_json::Value::Bool(syncing) => Ok(syncing),
        serde_json::Value::Object(_) => Ok(true),
        other => Err(format!("`{other}` is neither false nor a sync's progress")),
    }
}

pub(super) fn data(text: &str) -> Result<Vec<u8>, String> {
    let said: String = json(text)?;
    let digits = said.strip_prefix("0x").ok_or_else(|| format!("`{said}` is not 0x and hex"))?;
    hex::decode(digits).map_err(|error| error.to_string())
}

pub(super) fn quantity(value: U256) -> Json {
    Json::String(value.to_quantity())
}

pub(super) fn tag(block: BlockTag) -> Json {
    Json::String(block.to_string())
}

pub(super) fn hash(hash: B256) -> Json {
    Json::String(hash.to_hex())
}

pub(super) fn index(index: u64) -> Json {
    quantity(U256::from_u64(index))
}

pub(super) fn address(address: Address) -> Json {
    Json::String(address.to_hex())
}

#[cfg(test)]
#[path = "tests/query.rs"]
mod tests;
