//! Outcome-specific trading: splitting into sides and the fees an outcome fill pays.
pub mod fee;
pub mod split;

pub use fee::{UserFees, deployer_share, multiplier, user_fees, user_fees_request};
pub use split::{MAX_AMOUNT_PLACES, MergeOutcome, SplitOutcome};
