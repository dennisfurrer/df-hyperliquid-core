//! Hyperliquid vaults: vault details, deposit, withdraw, lockups, follower state.
//!
//! [`VaultTransfer`] moves USDC between the account's perp balance and a
//! vault; it is an L1 action, so an agent key may sign it for its master.
//! A deposit is locked: four days in HLP on both networks, and in any other
//! vault a day on mainnet and twenty seconds on testnet ([`lockup_ms`]). HLP
//! has its own address on each network ([`hlp`]).
//!
//! [`Ctx`] plans a move with the [`Key`] it needs, the [`Effect`] a read back
//! should show and when a deposit unlocks; [`VaultDetails::withdrawable_now`]
//! and [`VaultEquity::withdrawable`] say what may leave now.
#![deny(clippy::float_arithmetic)]

pub mod plan;
pub mod reads;
pub mod transfer;

pub use plan::{Balance, Change, Ctx, Effect, Key, Plan, Refused, key};
pub use reads::{
    Follower, LeadingVault, Relationship, VaultDetails, VaultEquity, leading_vaults, leading_vaults_request,
    user_vault_equities, user_vault_equities_request, vault_details, vault_details_request,
};
pub use transfer::{
    HLP_LOCKUP_MS, MAINNET_HLP, MAINNET_VAULT_LOCKUP_MS, TESTNET_HLP, TESTNET_VAULT_LOCKUP_MS, USD_DECIMALS,
    VaultTransfer, hlp, lockup_ms,
};
