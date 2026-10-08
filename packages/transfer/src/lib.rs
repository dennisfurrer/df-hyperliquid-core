//! Hyperliquid transfers to other people and out: usdSend, spotSend, sendAsset, withdraw3, and the ledger history behind them.
//!
//! Every action here but one is user-signed: [`UserAction`] gives each its
//! posted JSON, typed data, digest, wallet JSON and `/exchange` body, and only
//! the account's own key may sign it. [`AgentSendAsset`] is the L1 action an
//! agent signs to move between the account's own balances.
//!
//! [`Ctx`] plans a move: the action, its nonce, the [`Key`] it needs and the
//! [`Effect`] a read back should show. Also the checks before a send
//! ([`pre_transfer_check`], [`is_system_address`]), the ledger ([`ledger`],
//! and [`find`] for the row a send made), and the Arbitrum bridge's calldata
//! ([`arbitrum`]).
//!
//! **Core to HyperEVM** is a spot token sent to its system address
//! ([`Ctx::to_evm`]): `spotSend` from a separate account, `sendAsset` from a
//! pooled one. **HyperEVM to Core** is an EVM transaction to the same address,
//! built by hl-evm, not here.
//!
//! The network is named everywhere: HYPE and USDC have other indices and ids
//! on testnet (`hl_core::Network::hype` and `usdc`), and every other token
//! comes from the caller's `spotMeta` ([`SpotToken`]).
#![deny(clippy::float_arithmetic)]

pub mod actions;
pub mod amount;
pub mod arbitrum;
pub mod check;
pub mod keeping;
pub mod ledger;
pub mod plan;
pub mod token;

pub use actions::{
    AgentSendAsset, PERP_DEX, SPOT_DEX, SendAsset, SpotOut, SpotSend, UsdClassTransfer, UsdSend, Withdraw3,
};
pub use amount::{Amount, BadAmount, MAX_DECIMALS, clean_amount, cut};
pub use check::{
    HYPE_SYSTEM, PreTransferCheck, is_system_address, pre_transfer_check, pre_transfer_check_request, system_address,
};
pub use hl_core::{L1Action, UserAction};
pub use keeping::Keeping;
pub use ledger::{Delta, LedgerUpdate, find, ledger, ledger_request};
pub use plan::{Balance, Change, Ctx, Effect, Key, Plan, Refused, USD_DECIMALS, key};
pub use token::{EvmContract, HYPE_EVM_DECIMALS, MAINNET_HYPE, SpotToken, TESTNET_HYPE, hype};
