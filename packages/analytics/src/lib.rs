//! Who builds and deploys on Hyperliquid: the builder board, builder labels and fees, HIP-3 dexes and the builders routing perps, HIP-4 venues and their estimated fees, the revenue projection, payout plans and their ledger.
//!
//! A composite: pure computation over what the caller has read with the crates below it
//! (hl-hydromancer's builder matrix and day files, hl-hip3's dexes, hl-hip4's catalogue and
//! fills, hl-account's `referral`). Nothing here reads, sends or keeps a clock: whoever asks
//! says when now is ([`At`]). Money is exact, [`hl_core::Dec`] or integers, never a float.
//!
//! - [`builders`]: the board, every builder ranked by volume over a window
//!   ([`Board`]); the curated labels ([`labels`]); how a builder fee works, as numbers
//!   ([`codes`]); and what a builder code has been paid ([`Rewards`]).
#![deny(clippy::float_arithmetic)]

pub mod builders;
pub mod date;

pub use builders::{
    Board, Cap, Kind, Label, Ragged, Rewards, SHARE_OF_CAP, SHARE_UNITS, Stage, Standing, TENTHS_PER_BASIS_POINT,
    Window, codes, labels,
};
pub use date::{At, DAY_MS};
