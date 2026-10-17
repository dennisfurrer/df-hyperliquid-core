//! `settledOutcome`: what an outcome settled to, after `outcomeMeta` has dropped it; `null` until it
//! settles. Settlement pays out by itself: the first side gets `settleFraction` of the quote token,
//! the second the rest, with nothing to claim.

use std::collections::BTreeMap;

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::outcomes::meta::Outcome;
use crate::utils::{Dec, Json, OutcomeSide};
use serde::{Deserialize, Deserializer};

/// `{"type":"settledOutcome","outcome":..}`.
#[must_use]
pub fn settled_outcome_request(outcome: u32) -> String {
    info_request("settledOutcome", &[("outcome", Json::Number(outcome.to_string()))])
}

/// `None` until the outcome settles.
pub async fn settled_outcome(transport: &impl Transport, outcome: u32) -> Result<Option<SettledOutcome>, ApiError> {
    info(transport, settled_outcome_request(outcome)).await
}

/// An outcome as it settled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettledOutcome {
    /// The outcome as `outcomeMeta` listed it.
    pub spec: Outcome,
    /// What the first side was paid; read back as `0.0` or `1.0`.
    pub settle_fraction: Dec,
    /// The deployer's words on why.
    #[serde(default)]
    pub details: String,
    /// The question it was a leg of.
    #[serde(default)]
    pub question: Option<SettledQuestion>,
}

impl SettledOutcome {
    /// The side paid in full; `None` for a split payout.
    #[must_use]
    pub fn winner(&self) -> Option<OutcomeSide> {
        if self.settle_fraction == Dec::ONE {
            Some(OutcomeSide::Yes)
        } else if self.settle_fraction.is_zero() {
            Some(OutcomeSide::No)
        } else {
            None
        }
    }

    /// What one contract of `side` was paid, in the quote token.
    #[must_use]
    pub fn pays(&self, side: OutcomeSide) -> Dec {
        match side {
            OutcomeSide::Yes => self.settle_fraction,
            OutcomeSide::No => Dec::ONE.checked_sub(self.settle_fraction).unwrap_or(Dec::ZERO),
        }
    }
}

/// A settled leg's question: `{"question":{"active":823},"name","description"}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledQuestion {
    pub question: u32,
    /// The question's own state when the leg settled: `active`.
    pub state: String,
    pub name: String,
    pub description: String,
}

impl<'de> Deserialize<'de> for SettledQuestion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            question: BTreeMap<String, u32>,
            #[serde(default)]
            name: String,
            #[serde(default)]
            description: String,
        }

        let raw = Raw::deserialize(deserializer)?;
        let Some((state, question)) = raw.question.into_iter().next() else {
            return Err(serde::de::Error::custom("a settled leg's question names no id"));
        };
        Ok(SettledQuestion { question, state, name: raw.name, description: raw.description })
    }
}

#[cfg(test)]
#[path = "tests/settled.rs"]
mod tests;
