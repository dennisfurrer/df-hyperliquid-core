//! Hyperliquid transfers to other people and out: usdSend, spotSend, sendAsset, withdraw3, and the ledger history behind them.
//!
//! Every action here is user-signed: [`UserAction`] gives each its posted JSON,
//! typed data, digest, wallet JSON and `/exchange` body. Also the checks
//! before a send ([`pre_transfer_check`], [`is_system_address`]) and the
//! Arbitrum bridge's calldata ([`arbitrum`]).
#![deny(clippy::float_arithmetic)]

pub mod actions;
pub mod amount;
pub mod arbitrum;
pub mod check;
pub mod ledger;

pub use actions::{PERP_DEX, SPOT_DEX, SendAsset, SpotSend, UsdClassTransfer, UsdSend, Withdraw3};
pub use amount::{Amount, BadAmount, MAX_DECIMALS, clean_amount, cut};
pub use check::{
    HYPE_SYSTEM, PreTransferCheck, is_system_address, pre_transfer_check, pre_transfer_check_request, system_address,
};
pub use hl_core::UserAction;
pub use ledger::{Delta, LedgerUpdate, ledger, ledger_request};
