//! What an `outcomeDeploy` does: each register, settle and sub-deployer grant, the key it travels
//! under and the body it carries.

use crate::deploy::outcomes::actions::bounded::FeeScale;
use crate::deploy::outcomes::actions::grant::{Grant, Permission};
use crate::deploy::outcomes::actions::instance::Instance;
use crate::deploy::outcomes::actions::settlement::Settlement;
use crate::utils::sign::{Map, Value};

/// What an `outcomeDeploy` does, under the key naming it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Operation {
    /// `registerStandaloneOutcomeFromTemplate`: one outcome with two sides.
    RegisterStandalone {
        /// The template instance the outcome is made from.
        instance: Instance,
        /// The deployer's fee scale (`deployerFeeScale`), sent inside the instance.
        fee_scale: FeeScale,
    },
    /// `registerQuestionFromTemplate`: a question and its named outcomes, in
    /// the order given, which is the order they are shown in. The venue adds
    /// a fallback, so N named outcomes make N + 1.
    RegisterQuestion {
        /// The template instance the question is made from (`questionTemplateInstance`).
        question: Instance,
        /// The deployer's fee scale (`deployerFeeScale`), sent inside the question's instance.
        fee_scale: FeeScale,
        /// The named outcomes' template instances (`namedOutcomeTemplateInstances`), at least one.
        named: Vec<Instance>,
    },
    /// `registerAndAssociateNamedOutcomeFromTemplate`: one more named outcome
    /// on a live question, at the question's fee scale.
    RegisterAndAssociate {
        /// The question's id.
        question: u32,
        /// The new named outcome's template instance (`namedOutcomeTemplateInstance`).
        named: Instance,
    },
    /// `settleOutcome`. Pays out at once and cannot be undone.
    SettleOutcome(Settlement),
    /// `settleQuestion2`: every named outcome of the question not yet
    /// settled, exactly one of them at 1, the fallback left out. Pays out at
    /// once and cannot be undone.
    SettleQuestion {
        /// The question's id.
        question: u32,
        /// One settlement per named outcome left (`outcomeSettlements`).
        settlements: Vec<Settlement>,
        /// The question's name as `outcomeMeta` lists it, sent first in `nameAndDescription`.
        name: String,
        /// The question's description as `outcomeMeta` lists it, sent second in `nameAndDescription`.
        description: String,
    },
    /// `setSubDeployers`.
    SetSubDeployers(Vec<Grant>),
}

impl Operation {
    /// The key the operation travels under.
    #[must_use]
    pub const fn wire_name(&self) -> &'static str {
        match self {
            Operation::RegisterStandalone { .. } => "registerStandaloneOutcomeFromTemplate",
            Operation::RegisterQuestion { .. } => "registerQuestionFromTemplate",
            Operation::RegisterAndAssociate { .. } => "registerAndAssociateNamedOutcomeFromTemplate",
            Operation::SettleOutcome(_) => "settleOutcome",
            Operation::SettleQuestion { .. } => "settleQuestion2",
            Operation::SetSubDeployers(_) => "setSubDeployers",
        }
    }

    /// The grant a sub-deployer needs to send this; `None` for
    /// `setSubDeployers`, which no grant covers.
    #[must_use]
    pub const fn permission(&self) -> Option<Permission> {
        match self {
            Operation::RegisterStandalone { .. } => Some(Permission::RegisterStandalone),
            Operation::RegisterQuestion { .. } => Some(Permission::RegisterQuestion),
            Operation::RegisterAndAssociate { .. } => Some(Permission::RegisterAndAssociate),
            Operation::SettleOutcome(_) => Some(Permission::SettleOutcome),
            Operation::SettleQuestion { .. } => Some(Permission::SettleQuestion),
            Operation::SetSubDeployers(_) => None,
        }
    }

    /// Settling pays out at once and cannot be undone.
    #[must_use]
    pub const fn is_settlement(&self) -> bool {
        matches!(self, Operation::SettleOutcome(_) | Operation::SettleQuestion { .. })
    }

    /// The value sent under [`wire_name`](Self::wire_name).
    #[must_use]
    pub fn body(&self) -> Value {
        match self {
            Operation::RegisterStandalone { instance, fee_scale } => instance.wire_with(*fee_scale).into(),
            Operation::RegisterQuestion { question, fee_scale, named } => Map::new()
                .put("questionTemplateInstance", question.wire_with(*fee_scale))
                .put("namedOutcomeTemplateInstances", Value::array(named.iter().map(Instance::wire)))
                .into(),
            Operation::RegisterAndAssociate { question, named } => {
                Map::new().put("question", *question).put("namedOutcomeTemplateInstance", named.wire()).into()
            }
            Operation::SettleOutcome(settlement) => settlement.wire().into(),
            Operation::SettleQuestion { question, settlements, name, description } => Map::new()
                .put("question", *question)
                .put("outcomeSettlements", Value::array(settlements.iter().map(Settlement::wire)))
                .put("nameAndDescription", Value::array([name.clone(), description.clone()]))
                .into(),
            Operation::SetSubDeployers(grants) => Value::array(grants.iter().map(Grant::wire)),
        }
    }
}

#[cfg(test)]
#[path = "tests/operation.rs"]
mod tests;
