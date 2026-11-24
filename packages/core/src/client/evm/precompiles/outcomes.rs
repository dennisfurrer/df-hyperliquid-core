//! HIP-4 outcomes: an outcome's status, the only HIP-4 read on HyperEVM.

use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Dec;

/// A HIP-4 outcome as the chain sees it, the only HIP-4 read on HyperEVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutcomeStatus {
    /// The raw status byte; [`OutcomeStatus::state`] reads it.
    pub status: u8,
    /// 1e8 is 1.0; meaningful once settled.
    pub settled_value: u64,
    /// `u32::MAX` when the outcome stands alone or its question is unknown.
    pub question: u32,
}

/// What [`OutcomeStatus::status`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeState {
    /// Status 0: the outcome id was never created.
    NeverExisted,
    /// Status 1: the outcome trades and is not yet settled.
    Active,
    /// Settled, its value readable.
    Settled,
    /// Settled and pruned: its value is gone.
    Pruned,
    /// A status the docs do not name, kept as it came.
    Other(u8),
}

/// The places [`OutcomeStatus::settled_value`] is counted in.
pub const SETTLED_DECIMALS: u32 = 8;

impl OutcomeStatus {
    /// What [`OutcomeStatus::status`] says, by its code.
    #[must_use]
    pub const fn state(&self) -> OutcomeState {
        match self.status {
            0 => OutcomeState::NeverExisted,
            1 => OutcomeState::Active,
            2 => OutcomeState::Settled,
            3 => OutcomeState::Pruned,
            other => OutcomeState::Other(other),
        }
    }

    /// The question the outcome belongs to, if any.
    #[must_use]
    pub const fn in_question(&self) -> Option<u32> {
        match self.question {
            u32::MAX => None,
            question => Some(question),
        }
    }

    /// The settlement, 0 to 1, once settled and not pruned.
    #[must_use]
    pub fn settled(&self) -> Option<Dec> {
        match self.state() {
            OutcomeState::Settled => Dec::new(i128::from(self.settled_value), SETTLED_DECIMALS),
            _ => None,
        }
    }
}

/// `0x814`: an outcome's status.
#[must_use]
pub fn outcome_status(outcome: u32) -> Read<OutcomeStatus> {
    read(0x814, &[Arg::Uint(u64::from(outcome))], |answer| {
        let tokens = tokens(0x814, answer)?;
        let mut fields = Fields::of("outcomeStatus", &tokens)?;
        Ok(OutcomeStatus { status: fields.u8()?, settled_value: fields.u64()?, question: fields.u32()? })
    })
}

#[cfg(test)]
#[path = "tests/outcomes.rs"]
mod tests;
