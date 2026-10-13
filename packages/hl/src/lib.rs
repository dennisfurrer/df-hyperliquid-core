//! Hyperliquid in Rust, the whole family behind one dependency. Each crate is
//! a module here behind a feature, and can as well be used on its own:
//!
//! - [`core`]: vocabulary, exact decimals, signing, the rate-limit gate;
//! - markets: [`trade`] (orders and the ticket every market shares),
//!   [`perps`], [`spot`], [`hip3`], [`hip4`];
//! - money: [`account`], [`transfer`], [`lend`], [`stake`], [`vault`];
//! - data and chains: [`ws`], [`hydromancer`], [`evm`];
//! - composites: [`store`] (live state a page reads) and [`analytics`];
//! - venues on HyperEVM: [`papertrade`];
//! - wires: [`native`] or [`web`].
//!
//! A [`Client`] is one network: every crate's reads take it, since it is a
//! [`hl_core::Transport`] behind the rate-limit gate. With the `sign` feature
//! it also signs and sends actions for that network alone, each signature
//! recovered and checked against the signer before it is posted.

pub mod client;

pub use client::{Client, SendError};
#[cfg(feature = "account")]
pub use hl_account as account;
#[cfg(feature = "analytics")]
pub use hl_analytics as analytics;
pub use hl_core as core;
pub use hl_core::{Address, Dec, Network};
#[cfg(feature = "evm")]
pub use hl_evm as evm;
#[cfg(feature = "hip3")]
pub use hl_hip3 as hip3;
#[cfg(feature = "hip4")]
pub use hl_hip4 as hip4;
#[cfg(feature = "hydromancer")]
pub use hl_hydromancer as hydromancer;
#[cfg(feature = "lend")]
pub use hl_lend as lend;
#[cfg(feature = "native")]
pub use hl_native as native;
#[cfg(feature = "papertrade")]
pub use hl_papertrade as papertrade;
#[cfg(feature = "perps")]
pub use hl_perps as perps;
#[cfg(feature = "spot")]
pub use hl_spot as spot;
#[cfg(feature = "stake")]
pub use hl_stake as stake;
#[cfg(feature = "store")]
pub use hl_store as store;
#[cfg(feature = "trade")]
pub use hl_trade as trade;
#[cfg(feature = "transfer")]
pub use hl_transfer as transfer;
#[cfg(feature = "vault")]
pub use hl_vault as vault;
#[cfg(feature = "web")]
pub use hl_web as web;
#[cfg(feature = "ws")]
pub use hl_ws as ws;
