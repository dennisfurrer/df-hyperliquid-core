//! Builder fees: what an order pays the builder that routed it, the cap on each market kind, and
//! the setting an app routes its orders through.

pub mod fee;
pub mod setting;

pub use fee::{Builder, BuilderFee, OUTCOME_BUILDER_CAP, OverCap, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
pub use setting::{Approval, ApprovalRefused, Approved, BuilderSetting, FeeRule, SignedBy};
