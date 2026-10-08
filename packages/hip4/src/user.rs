//! `userOutcome`: an account turning quote tokens into one of each side of
//! an outcome, and back. Both may act for a subaccount through the body's
//! `vaultAddress`, which the hash covers.

use hl_core::{Dec, L1Action, Map, Value};

use crate::error::OutcomeError;

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

#[cfg(test)]
#[path = "tests/user.rs"]
mod tests;
