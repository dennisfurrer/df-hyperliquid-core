//! HIP-4 outcomes: coins and their asset ids, the price grid, orders on a side, `outcomeMeta`,
//! templates and the descriptions written from them, books on the grid, and settlements.

pub mod book;
pub mod coin;
pub mod error;
pub mod fields;
pub mod meta;
pub mod px;
pub mod settled;
pub mod template;

pub use book::{BadBook, Book, Level};
pub use coin::{Coin, PRICE_PLACES};
pub use error::OutcomeError;
pub use fields::Fields;
pub use meta::{Deployer, Outcome, OutcomeMeta, Question, SideSpec, outcome_meta, outcome_meta_request};
pub use px::{
    BadPx, BadQty, BadSignedQty, Mid, OBSERVED_PER_UNIT, OBSERVED_PLACES, Px, Qty, SignedQty, TICKS_PER_UNIT,
};
pub use settled::{SettledOutcome, SettledQuestion, settled_outcome, settled_outcome_request};
pub use template::{
    Family, Keyword, OnChain, OutcomeTemplate, RateMove, Remaining, Template, TemplateRole, Templates, deployer_limits,
    deployer_limits_request, fill, label, outcome_templates, outcome_templates_request,
};
