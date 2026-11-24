//! Templates: an outcome's name (`template:<id><version>`) gives its [`Family`]; `outcomeTemplates`
//! lists each template's title, roles and keywords, and how a row made from one reads
//! ([`OutcomeTemplate::on_chain`]); [`fill`] and [`label`] write a title and a side's name from the
//! keywords; `outcomeDeployerLimits` says what a venue may still register.

pub mod family;
pub mod limits;
pub mod listing;
pub mod title;

pub use family::{Family, RateMove, Template, UnknownFamily, VersionTooLarge};
pub use limits::{Remaining, deployer_limits, deployer_limits_request};
pub use listing::{
    Keyword, OnChain, OutcomeTemplate, TemplateRole, Templates, outcome_templates, outcome_templates_request,
};
pub use title::{fill, label};
