//! `outcomeDeploy`: every register and settle on a venue, made only in the shapes the venue takes,
//! a settlement only as `outcomeMeta` allows it.

use std::collections::BTreeSet;

use crate::deploy::outcomes::actions::bounded::{FeeScale, Fraction};
use crate::deploy::outcomes::actions::error::DeployError;
use crate::deploy::outcomes::actions::grant::Grant;
use crate::deploy::outcomes::actions::instance::Instance;
use crate::deploy::outcomes::actions::operation::Operation;
use crate::deploy::outcomes::actions::settlement::Settlement;
use crate::deploy::outcomes::actions::venue::Venue;
use crate::markets::outcomes::meta::{Outcome, OutcomeMeta};
use crate::utils::sign::{L1Action, Map};

/// `{"type":"outcomeDeploy","venue","operation":{<name>: ...}}`: every register
/// and settle, sent by the venue's deployer or an address it granted the
/// operation's [`Permission`](super::Permission).
///
/// The constructors check what [`Operation`] alone cannot: that a question
/// has a named outcome, that a grant list is not empty, and that a settlement
/// is one `outcomeMeta` allows, its words copied from it: a question settles
/// every named outcome it has left, exactly one at 1; its fallback never
/// settles alone; and its named outcome settles alone only at 0 or 1.
/// Settling pays out at once and cannot be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeDeploy {
    pub venue: Venue,
    pub operation: Operation,
}

impl OutcomeDeploy {
    #[must_use]
    pub fn register_standalone(venue: Venue, instance: Instance, fee_scale: FeeScale) -> Self {
        OutcomeDeploy { venue, operation: Operation::RegisterStandalone { instance, fee_scale } }
    }

    pub fn register_question(
        venue: Venue,
        question: Instance,
        fee_scale: FeeScale,
        named: Vec<Instance>,
    ) -> Result<Self, DeployError> {
        if named.is_empty() {
            return Err(DeployError::NoNamedOutcomes);
        }
        Ok(OutcomeDeploy { venue, operation: Operation::RegisterQuestion { question, fee_scale, named } })
    }

    #[must_use]
    pub fn register_and_associate(venue: Venue, question: u32, named: Instance) -> Self {
        OutcomeDeploy { venue, operation: Operation::RegisterAndAssociate { question, named } }
    }

    /// `settleOutcome` for `outcome` at `fraction`, its words copied from `meta`. An outcome no
    /// question holds settles to any fraction; a question's named outcome settles alone only to 0
    /// or 1 (testnet settled a question's legs one at a time as each was ruled out); a question's
    /// fallback never settles alone. Pays out at once and cannot be undone.
    pub fn settle_outcome(
        venue: Venue,
        meta: &OutcomeMeta,
        outcome: u32,
        fraction: Fraction,
    ) -> Result<Self, DeployError> {
        let row = meta.outcome(outcome).ok_or(DeployError::NotListed(outcome))?;
        if let Some(question) = meta.question_of(outcome) {
            if question.fallback_outcome == Some(outcome) {
                return Err(DeployError::Fallback { outcome, question: question.question });
            }
            if !fraction.is_whole() {
                return Err(DeployError::NotWhole(outcome));
            }
        }
        let settlement = settlement(row, fraction)?;
        Ok(OutcomeDeploy { venue, operation: Operation::SettleOutcome(settlement) })
    }

    /// `settleQuestion2` for `question`: every named outcome it has left, in the order `meta` lists
    /// them, `winner` at 1 and the rest at 0, the fallback left out, the question's words and each
    /// outcome's copied from `meta`. Pays out at once and cannot be undone.
    pub fn settle_question(venue: Venue, meta: &OutcomeMeta, question: u32, winner: u32) -> Result<Self, DeployError> {
        let asked = meta.question(question).ok_or(DeployError::NoQuestion(question))?;
        if !asked.unsettled().any(|leg| leg == winner) {
            return Err(DeployError::NotOpen { outcome: winner, question });
        }
        let mut seen = BTreeSet::new();
        let mut settlements = Vec::new();
        for leg in asked.unsettled() {
            if !seen.insert(leg) {
                return Err(DeployError::SettledTwice(leg));
            }
            let fraction = if leg == winner { Fraction::ONE } else { Fraction::ZERO };
            let row = meta.outcome(leg).ok_or(DeployError::NotListed(leg))?;
            settlements.push(settlement(row, fraction)?);
        }
        let (name, description) = (asked.name.clone(), asked.description.clone());
        Ok(OutcomeDeploy { venue, operation: Operation::SettleQuestion { question, settlements, name, description } })
    }

    pub fn set_sub_deployers(venue: Venue, grants: Vec<Grant>) -> Result<Self, DeployError> {
        if grants.is_empty() {
            return Err(DeployError::NoGrants);
        }
        Ok(OutcomeDeploy { venue, operation: Operation::SetSubDeployers(grants) })
    }
}

/// A row's settlement at `fraction`, its words as `outcomeMeta` lists them.
fn settlement(row: &Outcome, fraction: Fraction) -> Result<Settlement, DeployError> {
    row.settlement(fraction).ok_or(DeployError::NotTwoSided(row.outcome))
}

impl L1Action for OutcomeDeploy {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "outcomeDeploy")
            .put("venue", self.venue.as_str().to_owned())
            .put("operation", Map::new().put(self.operation.wire_name(), self.operation.body()))
    }
}

#[cfg(test)]
#[path = "tests/outcome_deploy.rs"]
mod tests;
