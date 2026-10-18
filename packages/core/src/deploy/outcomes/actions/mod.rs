//! What an outcome deployer signs, all L1 actions: claiming a venue, registering outcomes and questions from
//! templates, settling them with the words `outcomeMeta` lists, and letting other addresses do either. Values
//! are checked when made, not when signed. **Settling pays out at once and cannot be undone.**

pub mod activate;
pub mod bounded;
pub mod error;
pub mod grant;
pub mod instance;
pub mod keywords;
pub mod operation;
pub mod outcome_deploy;
pub mod settlement;
pub mod venue;

pub use activate::ActivateOutcomeDeployer;
pub use bounded::{FeeScale, Fraction};
pub use error::DeployError;
pub use grant::{Grant, Permission};
pub use instance::Instance;
pub use keywords::{BadValue, HORIZON_MS, Hint, Keywords, MAX_SHORT_LEN, MAX_VALUE_LEN, Problem};
pub use operation::Operation;
pub use outcome_deploy::OutcomeDeploy;
pub use settlement::Settlement;
pub use venue::Venue;

#[cfg(test)]
#[path = "tests/support.rs"]
mod support;
