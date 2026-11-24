//! Staking: an account's delegations and its staking totals, HYPE at 1e8.

use crate::client::evm::abi::{AbiError, Token};
use crate::client::evm::precompiles::fields::{Fields, tokens};
use crate::client::evm::precompiles::read::{Arg, Read, read};
use crate::utils::Address;

/// One stake delegation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delegation {
    /// The validator the stake is delegated to.
    pub validator: Address,
    /// HYPE at 1e8.
    pub amount: u64,
    /// Unix milliseconds.
    pub locked_until_timestamp: u64,
}

/// `0x804`: every delegation the account holds.
#[must_use]
pub fn delegations(user: Address) -> Read<Vec<Delegation>> {
    read(0x804, &[Arg::Address(user)], |answer| {
        let tokens = tokens(0x804, answer)?;
        let [Token::Array(items)] = tokens.as_slice() else {
            return Err(AbiError::Type("delegations: not a list".to_owned()));
        };
        items
            .iter()
            .map(|item| {
                let mut fields = Fields::of("delegations", std::slice::from_ref(item))?;
                Ok(Delegation {
                    validator: fields.address()?,
                    amount: fields.u64()?,
                    locked_until_timestamp: fields.u64()?,
                })
            })
            .collect()
    })
}

/// Staking totals, HYPE at 1e8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelegatorSummary {
    /// HYPE delegated to validators.
    pub delegated: u64,
    /// HYPE staked but not delegated.
    pub undelegated: u64,
    /// `totalPendingWithdrawal`: HYPE on its way out of staking.
    pub total_pending_withdrawal: u64,
    /// `nPendingWithdrawals`: the number of withdrawals not yet done.
    pub n_pending_withdrawals: u64,
}

/// `0x805`: the account's staking totals.
#[must_use]
pub fn delegator_summary(user: Address) -> Read<DelegatorSummary> {
    read(0x805, &[Arg::Address(user)], |answer| {
        let tokens = tokens(0x805, answer)?;
        let mut fields = Fields::of("delegatorSummary", &tokens)?;
        Ok(DelegatorSummary {
            delegated: fields.u64()?,
            undelegated: fields.u64()?,
            total_pending_withdrawal: fields.u64()?,
            n_pending_withdrawals: fields.u64()?,
        })
    })
}

#[cfg(test)]
#[path = "tests/staking.rs"]
mod tests;
