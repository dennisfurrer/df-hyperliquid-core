//! `outcomeMeta`: every live outcome, every question, and who deploys on which venue and whom it
//! lets send each operation. A settled outcome leaves the list for good.

use crate::client::{ApiError, Transport, info, info_request};
use crate::deploy::outcomes::actions::{Fraction, Settlement};
use crate::markets::outcomes::coin::Coin;
use crate::utils::{Address, Dec, OutcomeSide};
use serde::Deserialize;

/// `{"type":"outcomeMeta"}`.
#[must_use]
pub fn outcome_meta_request() -> String {
    info_request("outcomeMeta", &[])
}

/// Every live outcome, every question and every deployer.
pub async fn outcome_meta(transport: &impl Transport) -> Result<OutcomeMeta, ApiError> {
    info(transport, outcome_meta_request()).await
}

/// What `outcomeMeta` answers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct OutcomeMeta {
    /// Every live outcome.
    #[serde(default)]
    pub outcomes: Vec<Outcome>,
    /// Every question.
    #[serde(default)]
    pub questions: Vec<Question>,
    /// Every venue's deployer.
    #[serde(default)]
    pub deployers: Vec<Deployer>,
    /// The fee scale listed next to the lists (`feeScale`), read back as `1.0`; `None` when left out.
    #[serde(default)]
    pub fee_scale: Option<Dec>,
}

/// One outcome. A settlement must repeat its name, description and side names character for
/// character.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Outcome {
    /// The outcome's id.
    pub outcome: u32,
    /// The outcome's name, as written.
    #[serde(default)]
    pub name: String,
    /// The outcome's description, as written: `keyword:value` pairs joined by `|`.
    #[serde(default)]
    pub description: String,
    /// The two sides' names (`sideSpecs`), first side first.
    #[serde(default)]
    pub side_specs: Vec<SideSpec>,
    /// The token the outcome is quoted in (`quoteToken`), such as `USDC`.
    #[serde(default)]
    pub quote_token: Option<String>,
    /// The venue the outcome was deployed on; `None` when left out.
    #[serde(default)]
    pub venue: Option<String>,
    /// Read back as `1.0`; sent as `1`.
    #[serde(default)]
    pub deployer_fee_scale: Option<Dec>,
}

/// One side of an outcome as `outcomeMeta` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct SideSpec {
    /// The side's name, such as `Yes`.
    #[serde(default)]
    pub name: String,
}

impl Outcome {
    /// The coin of one side of this outcome.
    #[must_use]
    pub const fn coin(&self, side: OutcomeSide) -> Coin {
        Coin::new(self.outcome, side)
    }

    /// The settlement paying the first side `fraction`, with this outcome's name, description and
    /// side names exactly as listed. `None` unless it lists two sides. Settling pays out at once and
    /// cannot be undone.
    #[must_use]
    pub fn settlement(&self, fraction: Fraction) -> Option<Settlement> {
        let [first, second] = self.side_specs.as_slice() else {
            return None;
        };
        Some(Settlement {
            outcome: self.outcome,
            fraction,
            name: self.name.clone(),
            description: self.description.clone(),
            sides: [first.name.clone(), second.name.clone()],
        })
    }
}

/// A question: named outcomes, one of which settles at 1, and a fallback.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Question {
    /// The question's id.
    pub question: u32,
    /// The question's name, as written.
    #[serde(default)]
    pub name: String,
    /// The question's description, as written.
    #[serde(default)]
    pub description: String,
    /// The outcome that settles when no named outcome does (`fallbackOutcome`).
    #[serde(default)]
    pub fallback_outcome: Option<u32>,
    /// The named outcomes, in the order shown (`namedOutcomes`).
    #[serde(default)]
    pub named_outcomes: Vec<u32>,
    /// The named outcomes already settled one at a time (`settledNamedOutcomes`).
    #[serde(default)]
    pub settled_named_outcomes: Vec<u32>,
}

impl Question {
    /// The named outcomes not settled yet: what `settleQuestion2` covers.
    pub fn unsettled(&self) -> impl Iterator<Item = u32> + '_ {
        self.named_outcomes.iter().copied().filter(|outcome| !self.settled_named_outcomes.contains(outcome))
    }
}

/// A venue's deployer, and who it lets send each operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Deployer {
    /// The deployer's address.
    pub deployer: Address,
    /// The venue it deploys on.
    #[serde(default)]
    pub venue: Option<String>,
    /// Each operation the deployer lets others send, and who (`subDeployers`).
    #[serde(default)]
    pub sub_deployers: Vec<(String, Vec<Address>)>,
}

impl OutcomeMeta {
    /// The live outcome with the id `outcome`.
    #[must_use]
    pub fn outcome(&self, outcome: u32) -> Option<&Outcome> {
        self.outcomes.iter().find(|listed| listed.outcome == outcome)
    }

    /// The question with the id `question`.
    #[must_use]
    pub fn question(&self, question: u32) -> Option<&Question> {
        self.questions.iter().find(|listed| listed.question == question)
    }

    /// The question an outcome is named in or falls back for, if any.
    #[must_use]
    pub fn question_of(&self, outcome: u32) -> Option<&Question> {
        self.questions
            .iter()
            .find(|question| question.fallback_outcome == Some(outcome) || question.named_outcomes.contains(&outcome))
    }

    /// The deployer of a venue.
    #[must_use]
    pub fn deployer(&self, venue: &str) -> Option<&Deployer> {
        self.deployers.iter().find(|deployer| deployer.venue.as_deref() == Some(venue))
    }
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
