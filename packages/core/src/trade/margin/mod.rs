//! Leverage and margin on a perp: the leverage and mode a position takes, isolated margin in and
//! out, and the margin tables that cap leverage by size. The account-wide mode is
//! [`crate::account::settings::mode`].

pub mod isolated;
pub mod leverage;
pub mod tables;

pub use isolated::{UpdateIsolatedMargin, add_margin, margin, remove_margin};
pub use leverage::{MarginMode, UpdateLeverage, leverage, update_leverage, update_leverage_for};
pub use tables::{FLAT_TABLES_BELOW, LeverageError, MarginTable, MarginTier, checked_leverage};
