//! HyperCore's read precompiles: HyperCore state read from HyperEVM by an
//! `eth_call` to `0x...0800` through `0x...0814`, as `L1Read.sol` in
//! Hyperliquid's docs declares them.
//!
//! **The calldata is the ABI-encoded arguments with no selector.**
//! `L1Read.sol` calls each as `ADDRESS.staticcall(abi.encode(...))`, so a
//! selector would be read as part of the first argument.
//!
//! The numbers are HyperCore's integers, unscaled:
//! - a perp's `markPx`, `oraclePx` and `bbo` are in `10^-(6 - szDecimals)`
//!   ([`perp_price`]), a spot pair's `spotPx` and `bbo` in
//!   `10^-(8 - szDecimals)` of its base token ([`spot_price`]);
//! - a position's `szi` is in the perp's size decimals;
//! - USDC amounts (`entryNtl`, `equity`, `withdrawable`, margins) are 1e6;
//! - token balances are in the token's `weiDecimals`, HYPE staking in 1e8;
//! - an outcome's `settledValue` is 1e8 for 1.0.
//!
//! An invalid input (an unknown asset or vault) is refused and uses all the
//! gas given. Gas is `2000 + 65 * (input + output bytes)`.

pub mod account;
pub mod assets;
mod fields;
pub mod lending;
pub mod outcomes;
pub mod prices;
pub mod read;
pub mod staking;
pub mod table;

pub use account::{
    AccountMarginSummary, Position, SpotBalance, UserVaultEquity, account_margin_summary, core_user_exists, position,
    position2, spot_balance, user_vault_equity, withdrawable,
};
pub use assets::{
    PerpAssetInfo, SpotInfo, TokenInfo, TokenSupply, UserBalance, perp_asset_info, spot_info, token_info, token_supply,
};
pub use lending::{
    BasisAndValue, BorrowLendReserveState, BorrowLendUserState, borrow_lend_reserve_state, borrow_lend_user_state,
};
pub use outcomes::{OutcomeState, OutcomeStatus, SETTLED_DECIMALS, outcome_status};
pub use prices::{Bbo, bbo, l1_block_number, mark_px, oracle_px, perp_price, spot_price, spot_px};
pub use read::{Read, ReadError};
pub use staking::{Delegation, DelegatorSummary, delegations, delegator_summary};
pub use table::{ALL, Spec, address, spec};

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
