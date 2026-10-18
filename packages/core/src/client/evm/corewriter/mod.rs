//! CoreWriter, the write path from HyperEVM to HyperCore: a system contract
//! at `0x3333...3333` (`CoreWriter.sol` in the docs):
//!
//! ```text
//! event RawAction(address indexed user, bytes data);
//! function sendRawAction(bytes calldata data) external;
//! ```
//!
//! `data` is the encoding version (one byte, 1), the action id (a big-endian
//! `uint24`), then the action's fields ABI-encoded with no selector.
//!
//! - The action acts for `msg.sender`: an agent key sending it acts for the
//!   agent's own account, never for the account it trades for.
//! - The transaction's value must be 0: value voids the action silently, with
//!   no log.
//! - Orders and vault transfers are delayed on chain by a few seconds.
//!
//! Units: `limitPx` and `sz` are 1e8 whatever the market; `usd` and `ntl`
//! are perp USDC at 1e6; staking and delegation `wei` are HYPE at 1e8; any
//! other `wei` is in the token's `weiDecimals`; `maxFeeRate` is in tenths of a
//! basis point (10 is 0.01%).

pub mod action;
pub mod decode;
pub mod options;
pub mod raw;
pub mod table;

pub use action::Action;
pub use decode::BadAction;
pub use options::{Abstraction, BorrowLendOp, FinalizeVariant, OutcomeOp, Tif};
pub use raw::{RawAction, raw_action_logs};
pub use table::{
    CORE_WRITER, ENCODING_VERSION, GAP, ORDER_DECIMALS, RAW_ACTION_TOPIC, SEND_RAW_ACTION, SPECS, SPOT_DEX, Spec, spec,
};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
