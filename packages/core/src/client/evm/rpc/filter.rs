//! [`LogFilter`]: what `eth_getLogs` matches.

use crate::client::evm::rpc::BlockTag;
use crate::client::evm::u256::B256;
use crate::utils::{Address, Json};

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
    pub(super) fn json(&self) -> Json {
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

#[cfg(test)]
#[path = "tests/filter.rs"]
mod tests;
