//! `userOutcome`: an account turning quote tokens into one of each side of
//! an outcome, and back; merging a Yes of every outcome of a question back into quote tokens; and
//! turning an outcome's No into a Yes of every other outcome of its question. Each may act for a
//! subaccount through the body's `vaultAddress`, which the hash covers.

use crate::markets::outcomes::error::OutcomeError;
use crate::utils::Dec;
use crate::utils::sign::{L1Action, Map, Value};

/// The most places a quote amount may have.
pub const MAX_AMOUNT_PLACES: u32 = 8;

fn amount(value: Dec) -> Result<Dec, OutcomeError> {
    match value.is_positive() && value.places() <= MAX_AMOUNT_PLACES {
        true => Ok(value),
        false => Err(OutcomeError::Amount(value)),
    }
}

/// `{"type":"userOutcome","splitOutcome":{"outcome","amount"}}`: `amount` of
/// the quote token into that many of each side. The amount goes as a decimal
/// with no trailing zeros, so `12.50` and `12.5` are one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitOutcome {
    outcome: u32,
    amount: Dec,
}

impl SplitOutcome {
    pub fn new(outcome: u32, amount_of_quote: Dec) -> Result<SplitOutcome, OutcomeError> {
        Ok(SplitOutcome { outcome, amount: amount(amount_of_quote)? })
    }
}

impl L1Action for SplitOutcome {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "userOutcome")
            .put("splitOutcome", Map::new().put("outcome", self.outcome).put("amount", self.amount.to_wire()))
    }
}

/// [`SplitOutcome`] undone: as many of each side back into the quote token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeOutcome {
    outcome: u32,
    amount: Option<Dec>,
}

impl MergeOutcome {
    pub fn new(outcome: u32, amount_of_quote: Dec) -> Result<MergeOutcome, OutcomeError> {
        Ok(MergeOutcome { outcome, amount: Some(amount(amount_of_quote)?) })
    }

    /// Every pair the account holds: the amount goes as nil, not a figure.
    #[must_use]
    pub const fn all(outcome: u32) -> MergeOutcome {
        MergeOutcome { outcome, amount: None }
    }
}

impl L1Action for MergeOutcome {
    fn action(&self) -> Map {
        let amount = match self.amount {
            Some(amount) => Value::from(amount.to_wire()),
            None => Value::Nil,
        };
        Map::new()
            .put("type", "userOutcome")
            .put("mergeOutcome", Map::new().put("outcome", self.outcome).put("amount", amount))
    }
}

/// `{"type":"userOutcome","mergeQuestion":{"question","amount"}}`: `amount` Yes shares of each
/// outcome of the question back into that many quote tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeQuestion {
    question: u32,
    amount: Option<Dec>,
}

impl MergeQuestion {
    /// `amount` of each outcome's Yes, at most eight places.
    pub fn new(question: u32, amount: Dec) -> Result<MergeQuestion, OutcomeError> {
        Ok(MergeQuestion { question, amount: Some(self::amount(amount)?) })
    }

    /// Every full set the account holds: the amount goes as nil, not a figure.
    #[must_use]
    pub const fn all(question: u32) -> MergeQuestion {
        MergeQuestion { question, amount: None }
    }
}

impl L1Action for MergeQuestion {
    fn action(&self) -> Map {
        let amount = match self.amount {
            Some(amount) => Value::from(amount.to_wire()),
            None => Value::Nil,
        };
        Map::new()
            .put("type", "userOutcome")
            .put("mergeQuestion", Map::new().put("question", self.question).put("amount", amount))
    }
}

/// `{"type":"userOutcome","negateOutcome":{"question","outcome","amount"}}`: `amount` No shares of
/// `outcome` into as many Yes shares of every other outcome of `question`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegateOutcome {
    question: u32,
    outcome: u32,
    amount: Dec,
}

impl NegateOutcome {
    /// `amount` of the outcome's No, at most eight places.
    pub fn new(question: u32, outcome: u32, amount: Dec) -> Result<NegateOutcome, OutcomeError> {
        Ok(NegateOutcome { question, outcome, amount: self::amount(amount)? })
    }
}

impl L1Action for NegateOutcome {
    fn action(&self) -> Map {
        Map::new().put("type", "userOutcome").put(
            "negateOutcome",
            Map::new().put("question", self.question).put("outcome", self.outcome).put("amount", self.amount.to_wire()),
        )
    }
}

#[cfg(test)]
#[path = "tests/split.rs"]
mod tests;
