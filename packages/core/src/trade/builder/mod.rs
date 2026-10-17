//! Builder fees: what an order pays the builder that routed it, the cap on each market kind, and
//! the setting orders are routed through with what an order needs approved first.

pub mod fee;
pub mod setting;

pub use fee::{Builder, BuilderFee, OUTCOME_BUILDER_CAP, OverCap, PERP_BUILDER_CAP, SPOT_BUILDER_CAP};
pub use setting::{Approval, ApprovalRefused, Approved, BuilderSetting, FeeRule};
