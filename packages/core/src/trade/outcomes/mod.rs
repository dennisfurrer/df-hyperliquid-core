//! Outcome-specific trading: splitting into sides and merging back, and what an outcome fill is
//! billed.
pub mod fee;
pub mod split;

pub use fee::{billed, billed_for, deployer_share, multiplier, rate};
pub use split::{MAX_AMOUNT_PLACES, MergeOutcome, SplitOutcome};
