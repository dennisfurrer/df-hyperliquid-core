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
//! - [`hip3`]: every perp dex and how its deployer set it up ([`Setup`]), what its assets are
//!   doing ([`Activity`]), and the builders that route perps on a day ([`Routing`]).
//! - [`hip4`]: who lists what ([`Venue`]) and from which templates ([`Uses`]), and what a deployer
//!   earns. No endpoint reports that, so it is an [`Estimate`] wherever it appears: from the
//!   deployer's own ledger ([`Earned`]) or from fills by the fee model ([`Paid`], [`Takes`]).
//! - [`projection`]: the revenue projection, period by period ([`Projection`], [`Period`]), and
//!   the assumptions it is made of.
#![deny(clippy::float_arithmetic)]

pub mod builders;
pub mod date;
pub mod estimate;
pub mod hip3;
pub mod hip4;
pub mod projection;

pub use builders::{
    Board, Cap, Kind, Label, Ragged, Rewards, SHARE_OF_CAP, SHARE_UNITS, Stage, Standing, TENTHS_PER_BASIS_POINT,
    Window, codes, labels,
};
pub use date::{At, DAY_MS};
pub use estimate::{Basis, Estimate};
pub use hip3::{Activity, Routing, Setup};
pub use hip4::{Earned, First, LedgerRow, Paid, Take, Takes, Uses, Venue};
pub use projection::{Period, Projection, Totals};
