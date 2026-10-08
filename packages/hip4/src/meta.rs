//! `outcomeMeta`: every live outcome, every question, and who deploys on
//! which venue. A settled outcome leaves the list for good.

use hl_core::{Address, ApiError, Dec, OutcomeSide, Transport, info, info_request};
use serde::Deserialize;

use crate::{
    coin::Coin,
    deploy::{Fraction, Settlement},
};

/// `{"type":"outcomeMeta"}`.
#[must_use]
pub fn outcome_meta_request() -> String {
    info_request("outcomeMeta", &[])
}

pub async fn outcome_meta(transport: &impl Transport) -> Result<OutcomeMeta, ApiError> {
    info(transport, outcome_meta_request()).await
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeMeta {
    #[serde(default)]
    pub outcomes: Vec<Outcome>,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub deployers: Vec<Deployer>,
    #[serde(default)]
    pub fee_scale: Option<Dec>,
}

/// One outcome. Its name, description and side names are what a settlement
/// must repeat, character for character.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub outcome: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub side_specs: Vec<SideSpec>,
    #[serde(default)]
    pub quote_token: Option<String>,
    #[serde(default)]
    pub venue: Option<String>,
    /// Read back as `1.0`; sent as `1`.
    #[serde(default)]
    pub deployer_fee_scale: Option<Dec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SideSpec {
    #[serde(default)]
    pub name: String,
}

impl Outcome {
    #[must_use]
    pub const fn coin(&self, side: OutcomeSide) -> Coin {
        Coin::new(self.outcome, side)
    }

    /// The settlement paying the first side `fraction`, with this outcome's
    /// name, description and side names exactly as listed. `None` unless it
    /// lists two sides. Settling pays out at once and cannot be undone.
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
pub struct Question {
    pub question: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub fallback_outcome: Option<u32>,
    #[serde(default)]
    pub named_outcomes: Vec<u32>,
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
pub struct Deployer {
    pub deployer: Address,
    #[serde(default)]
    pub venue: Option<String>,
    #[serde(default)]
    pub sub_deployers: Vec<(String, Vec<Address>)>,
}

impl OutcomeMeta {
    #[must_use]
    pub fn outcome(&self, outcome: u32) -> Option<&Outcome> {
        self.outcomes.iter().find(|listed| listed.outcome == outcome)
    }

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

    /// The venue `who` names: a venue's own name, case aside, or its
    /// deployer's address.
    #[must_use]
    pub fn venue_of(&self, who: &str) -> Option<&str> {
        let who = who.trim();
        match who.parse::<Address>() {
            Ok(address) => self.deployers.iter().find(|one| one.deployer == address)?.venue.as_deref(),
            Err(_) => self
                .deployers
                .iter()
                .find_map(|one| one.venue.as_deref().filter(|venue| venue.eq_ignore_ascii_case(who))),
        }
    }
}

/// One address a deployer lets send operations, and which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubDeployer {
    pub address: Address,
    /// The operations' wire names, in the order the venue lists them.
    pub actions: Vec<String>,
}

impl SubDeployer {
    #[must_use]
    pub fn settles(&self) -> bool {
        self.actions.iter().any(|action| action.starts_with("settle"))
    }

    #[must_use]
    pub fn registers(&self) -> bool {
        self.actions.iter().any(|action| action.starts_with("register"))
    }
}

impl Deployer {
    /// The table the venue keeps by operation, read by address instead: the
    /// addresses granted most first.
    #[must_use]
    pub fn keys(&self) -> Vec<SubDeployer> {
        let mut found: Vec<SubDeployer> = Vec::new();
        for (action, addresses) in &self.sub_deployers {
            for address in addresses {
                match found.iter_mut().find(|key| key.address == *address) {
                    Some(key) => key.actions.push(action.clone()),
                    None => found.push(SubDeployer { address: *address, actions: vec![action.clone()] }),
                }
            }
        }
        found.sort_by(|a, b| (b.actions.len(), b.address).cmp(&(a.actions.len(), a.address)));
        found
    }
}

#[cfg(test)]
#[path = "tests/meta.rs"]
mod tests;
