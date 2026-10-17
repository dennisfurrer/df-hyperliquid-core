//! Deploying HIP-4 outcome markets: the actions a deployer signs, checked against `outcomeMeta`
//! where a settlement depends on it.
pub mod actions;

pub use actions::{
    ActivateOutcomeDeployer, BadValue, DeployError, FeeScale, Fraction, Grant, HORIZON_MS, Hint, Instance, Keywords,
    MAX_SHORT_LEN, MAX_VALUE_LEN, Operation, OutcomeDeploy, Permission, Problem, Settlement, Venue,
};
