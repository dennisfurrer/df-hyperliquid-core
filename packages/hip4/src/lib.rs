//! HIP-4 outcome markets: outcome meta, outcome asset ids, outcome actions, settlement.
//!
//! An outcome has two sides, each a [`Coin`] traded like a spot pair: its
//! asset id is `100000000 + 10 * outcome + side`, its price a probability on a
//! 0.00001 grid, its size whole contracts. A market order is `FrontendMarket`
//! at the edge of the range ([`Coin::market`]), never `Gtc`.
//!
//! Every action here is an L1 action. [`SplitOutcome`] and [`MergeOutcome`]
//! turn quote tokens into one of each side and back; [`OutcomeDeploy`] and
//! [`ActivateOutcomeDeployer`] are a deployer's. Settling pays out at once and
//! cannot be undone.
#![deny(clippy::float_arithmetic)]

pub mod book;
pub mod coin;
pub mod deploy;
pub mod error;
pub mod fee;
pub mod fields;
pub mod fill;
pub mod market;
pub mod meta;
pub mod px;
pub mod quote;
pub mod scan;
pub mod settled;
pub mod template;
pub mod time;
pub mod user;

pub use coin::{Coin, MAX_PRICE, MIN_PRICE, PRICE_PLACES, outcome_price, outcome_size};
pub use deploy::{
    ActivateOutcomeDeployer, BadValue, DeployError, FeeScale, Fraction, Grant, HORIZON_MS, Hint, Instance, Keywords,
    MAX_SHORT_LEN, MAX_VALUE_LEN, Operation, OutcomeDeploy, Permission, Problem, Settlement, Venue,
};
pub use error::OutcomeError;
pub use hl_core::OutcomeSide;
pub use hl_trade::{Side, Tif};
pub use meta::{Deployer, Outcome, OutcomeMeta, Question, SideSpec, outcome_meta, outcome_meta_request};
pub use user::{MAX_AMOUNT_PLACES, MergeOutcome, SplitOutcome};
