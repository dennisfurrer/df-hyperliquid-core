//! Moving money: sends, withdrawals, deposits through the Arbitrum bridge, moves between perps and spot, and HyperCore to HyperEVM.
#![deny(clippy::float_arithmetic)]

pub mod actions;
pub mod amount;
pub mod arbitrum;
pub mod check;
pub mod erc20;
pub mod keeping;
pub mod ledger;
pub mod link;
pub mod plan;
pub mod system;
pub mod token;

pub use crate::utils::sign::{L1Action, UserAction};
pub use actions::{
    AgentSendAsset, PERP_DEX, SPOT_DEX, SendAsset, SpotOut, SpotSend, UsdClassTransfer, UsdSend, Withdraw3,
};
pub use amount::{Amount, BadAmount, MAX_DECIMALS, clean_amount, cut};
pub use check::{
    HYPE_SYSTEM, PreTransferCheck, is_system_address, pre_transfer_check, pre_transfer_check_request, system_address,
};
pub use keeping::Keeping;
pub use ledger::{Delta, LedgerUpdate, find, ledger, ledger_request};
pub use link::Linked;
pub use plan::{Balance, Change, Ctx, Effect, Key, Plan, Refused, USD_DECIMALS, key};
pub use system::system_token;
pub use token::{EvmContract, HYPE_EVM_DECIMALS, MAINNET_HYPE, SpotToken, TESTNET_HYPE, hype};
