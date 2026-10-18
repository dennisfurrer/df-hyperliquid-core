//! `RawAction` logs: what CoreWriter logged and who sent it, read back into an
//! [`Action`], and the filter that finds them.

use crate::client::evm::abi::{self, Token};
use crate::client::evm::corewriter::action::Action;
use crate::client::evm::corewriter::decode::BadAction;
use crate::client::evm::corewriter::table::{CORE_WRITER, RAW_ACTION_TOPIC};
use crate::client::evm::records::Log;
use crate::client::evm::rpc::{BlockTag, LogFilter};
use crate::utils::Address;

/// A `RawAction` log: who sent it, and what it carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAction {
    /// `msg.sender` of `sendRawAction`, the account HyperCore acts for.
    pub user: Address,
    pub version: u8,
    pub id: u32,
    /// The `data` argument as it was logged.
    pub payload: Vec<u8>,
    /// The payload read; an id outside the table keeps the log readable.
    pub action: Result<Action, BadAction>,
}

impl RawAction {
    /// A log of CoreWriter's, read. Any other log is [`BadAction::NotRawAction`].
    pub fn from_log(log: &Log) -> Result<RawAction, BadAction> {
        let user = match log.topics.as_slice() {
            [topic, user, ..] if log.address == CORE_WRITER && *topic == RAW_ACTION_TOPIC => user.to_address(),
            _ => None,
        }
        .ok_or(BadAction::NotRawAction)?;
        let payload = match abi::decode(&abi::params("bytes")?, &log.data)?.as_slice() {
            [Token::Bytes(bytes)] => bytes.clone(),
            _ => return Err(BadAction::NotRawAction),
        };
        let (version, id) = match payload.split_first_chunk::<4>() {
            Some((header, _)) => (header[0], u32::from_be_bytes([0, header[1], header[2], header[3]])),
            None => (0, 0),
        };
        Ok(RawAction { user, version, id, action: Action::decode(&payload), payload })
    }
}

/// The `eth_getLogs` filter for every action sent through CoreWriter between
/// two blocks. HyperEVM serves at most 50 blocks a query.
#[must_use]
pub fn raw_action_logs(from: BlockTag, to: BlockTag) -> LogFilter {
    LogFilter {
        from_block: Some(from),
        to_block: Some(to),
        address: vec![CORE_WRITER],
        topics: vec![Some(vec![RAW_ACTION_TOPIC])],
        ..LogFilter::default()
    }
}

#[cfg(test)]
#[path = "tests/raw.rs"]
mod tests;
