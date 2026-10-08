//! Hyperliquid vaults: vault details, deposit, withdraw, lockups, follower state.
//!
//! [`VaultTransfer`] moves USDC between the account's perp balance and a
//! vault; it is an L1 action, so an agent key may sign it for its master.
//! A deposit is locked: four days in HLP, a day in any other vault.
#![deny(clippy::float_arithmetic)]

pub mod reads;
pub mod transfer;

pub use reads::{
    Follower, LeadingVault, VaultDetails, VaultEquity, leading_vaults, leading_vaults_request, user_vault_equities,
    user_vault_equities_request, vault_details, vault_details_request,
};
pub use transfer::{HLP, HLP_LOCKUP_MS, USD_DECIMALS, VAULT_LOCKUP_MS, VaultTransfer, lockup_ms};
