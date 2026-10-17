//! HIP-4 outcomes: coins, the price grid, `outcomeMeta` and templates in words, books, quotes, set scans, settlements.
#![deny(clippy::float_arithmetic)]

pub mod book;
pub mod coin;
pub mod error;
pub mod fields;
pub mod market;
pub mod meta;
pub mod px;
pub mod quote;
pub mod scan;
pub mod settled;
pub mod template;
pub mod time;

pub use crate::trade::{Side, Tif};
pub use crate::utils::OutcomeSide;
pub use book::{BadBook, Book, Level, book_request};
pub use coin::{Coin, MAX_PRICE, MIN_PRICE, PRICE_PLACES, outcome_price, outcome_size};
pub use error::OutcomeError;
pub use fields::Fields;
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
