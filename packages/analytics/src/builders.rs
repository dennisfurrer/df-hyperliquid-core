//! Builders: everyone who routes orders for a fee. The board ranks them by outcome volume over a
//! window ([`board`]); the curated list says who an address is ([`labels`]); [`codes`] says how
//! a builder fee works, as numbers; [`rewards`] reads what a builder code has been paid.

pub mod board;
pub mod codes;
pub mod labels;
pub mod rewards;

pub use board::{Board, Ragged, SHARE_UNITS, Standing, Window};
pub use codes::{Cap, SHARE_OF_CAP, TENTHS_PER_BASIS_POINT};
pub use labels::{Kind, Label};
pub use rewards::{Rewards, Stage};
