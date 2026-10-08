//! HIP-4 outcome markets: outcome coins and asset ids, the price grid, outcome meta and templates read in words, the complete-set scan, holdings, fills and fees, outcome actions, deployer checks, settlement.
//!
//! An outcome has two sides, each a [`Coin`] traded like a spot pair: its
//! asset id is `100000000 + 10 * outcome + side`, its price a probability on a
//! 0.00001 grid ([`Px`], D1), its size whole contracts ([`Qty`], D2). A market
//! order is `FrontendMarket` at the edge of the range ([`Coin::market`]) or
//! bounded past a reference ([`Coin::market_within`]), never `Gtc`.
//!
//! Every action here is an L1 action. [`SplitOutcome`] and [`MergeOutcome`]
//! turn quote tokens into one of each side and back; [`OutcomeDeploy`] and
//! [`ActivateOutcomeDeployer`] are a deployer's, and [`flow`] checks each step
//! against `outcomeMeta` and the venue's templates before it is made.
//! Settling pays out at once and cannot be undone.
//!
//! Reading the venue: [`market`] reads `outcomeMeta` into a [`Catalog`] of
//! markets and questions in words; [`template`] reads `outcomeTemplates` and
//! a deployer's limits; [`scan`] prices complete sets, before fees; [`quote`]
//! says whether a mid is a probability; [`fill`] reads holdings from the spot
//! side and fills beside them (D9, D10); [`fee`] models what a fill is billed
//! (D11); [`settled`] reads what an outcome settled to. Nothing here keeps a
//! clock: whoever asks whether a deadline has passed says when now is.
#![deny(clippy::float_arithmetic)]

pub mod book;
pub mod coin;
pub mod deploy;
pub mod error;
pub mod fee;
pub mod fields;
pub mod fill;
pub mod flow;
pub mod market;
pub mod meta;
pub mod px;
pub mod quote;
pub mod scan;
pub mod settled;
pub mod template;
pub mod time;
pub mod user;

pub use book::{Book, Level, book_request};
pub use coin::{Coin, MAX_PRICE, MIN_PRICE, PRICE_PLACES, outcome_price, outcome_size};
pub use deploy::{
    ActivateOutcomeDeployer, BadValue, DeployError, FeeScale, Fraction, Grant, HORIZON_MS, Hint, Instance, Keywords,
    MAX_SHORT_LEN, MAX_VALUE_LEN, Operation, OutcomeDeploy, Permission, Problem, Settlement, Venue,
};
pub use error::OutcomeError;
pub use fee::{UserFees, deployer_share, multiplier, user_fees, user_fees_request};
pub use fields::Fields;
pub use fill::{
    Check, Direction, Fill, Holding, OpenOrder, Position, RawFill, RawOrder, SpotBalances, SpotRow, Summary,
    corroborate, holdings, positions, summarize,
};
pub use hl_core::OutcomeSide;
pub use hl_trade::{Side, Tif};
pub use market::{Catalog, Details, Market, Pair, Sides, Status, contains};
pub use meta::{Deployer, Outcome, OutcomeMeta, Question, SideSpec, SubDeployer, outcome_meta, outcome_meta_request};
pub use px::{
    BadPx, BadQty, BadSignedQty, MIN_NOTIONAL, Mid, OBSERVED_PER_UNIT, OBSERVED_PLACES, Px, Qty, SignedQty,
    TICKS_PER_UNIT, min_shares,
};
pub use quote::{Quality, assess, depth_within};
pub use scan::{CREDIBLE_SPREAD, Touch, executable, gaps, sets};
pub use settled::{SettledOutcome, SettledQuestion, settled_outcome, settled_outcome_request};
pub use template::{
    DeployerLimits, Family, Keyword, OutcomeTemplate, Pattern, Patterns, RateMove, Remaining, Template, TemplateRole,
    Templates, deployer_limits, deployer_limits_request, outcome_templates, outcome_templates_request, patterns,
};
pub use time::Stamp;
pub use user::{MAX_AMOUNT_PLACES, MergeOutcome, SplitOutcome};
