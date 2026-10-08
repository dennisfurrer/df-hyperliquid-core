//! What a deployer's address was paid, read off its own ledger: the USDC it holds less what the
//! rows of `userNonFundingLedgerUpdates` explain. On a deployer's address the rest is what its
//! markets paid it, which no endpoint reports, so it is an [`Estimate`] on the ledger's basis; a
//! row this build cannot read makes it unknown rather than wrong. Beside it, what `referral`
//! says its builder code was paid (hl-account's `Referral`, read as [`Rewards`]).
//!
//! The rows are hl-core's `LedgerUpdate`, every type read by its fields; what each moved is
//! worked out here, by hip4-tui's rules.

use hl_core::{
    Address, Dec,
    ledger::{Delta, LedgerUpdate},
};

use crate::{
    builders::Rewards,
    estimate::{Basis, Estimate},
    math::add,
};

/// One row of `userNonFundingLedgerUpdates`: hl-core's.
pub type LedgerRow = LedgerUpdate;

/// What a row did: hl-core's.
pub type LedgerDelta = Delta;

/// What a ledger row moved for one account.
pub trait Moved {
    /// The USDC this row moved into `me`'s account (out, below zero), less any USDC fee it
    /// charged `me`; `None` for a row this build does not read.
    fn moved(&self, me: Address) -> Option<Dec>;
}

impl Moved for LedgerUpdate {
    fn moved(&self, me: Address) -> Option<Dec> {
        let to_me = |amount: Dec, destination: Address| if destination == me { amount } else { amount.negate() };
        let quote = |token: &str, amount: Dec| if token == QUOTE { amount } else { Dec::ZERO };
        match &self.delta {
            Delta::Send { user, destination, token, amount, fee, fee_token, .. }
            | Delta::SpotTransfer { user, destination, token, amount, fee, fee_token, .. } => {
                let charged = if fee_token == QUOTE && *user == me { *fee } else { Dec::ZERO };
                to_me(quote(token, *amount), *destination).checked_sub(charged)
            }
            Delta::InternalTransfer { usdc, destination, .. } | Delta::SubAccountTransfer { usdc, destination, .. } => {
                Some(to_me(*usdc, *destination))
            }
            Delta::Deposit { usdc } => Some(*usdc),
            Delta::RewardsClaim { amount, token } => Some(quote(token, *amount)),
            Delta::Withdraw { usdc, .. } | Delta::VaultDeposit { usdc, .. } => Some(usdc.negate()),
            Delta::VaultCreate { usdc, fee, .. } => usdc.checked_add(*fee).map(Dec::negate),
            Delta::AccountClassTransfer { usdc, to_perp } => Some(if *to_perp { usdc.negate() } else { *usdc }),
            Delta::CStakingTransfer { .. } | Delta::SpotGenesis { .. } => Some(Dec::ZERO),
            Delta::VaultWithdraw { .. }
            | Delta::AccountActivationGas { .. }
            | Delta::DeployGasAuction { .. }
            | Delta::Other { .. } => None,
        }
    }
}

/// Every row's USDC summed for `me`, and whether every row read.
#[must_use]
pub fn explained(rows: &[LedgerRow], me: Address) -> (Dec, bool) {
    rows.iter().fold((Dec::ZERO, true), |(total, exact), row| match row.moved(me) {
        Some(moved) => (add(total, moved), exact),
        None => (total, false),
    })
}

/// The quote every figure here is counted in.
const QUOTE: &str = "USDC";

/// A deployer's address: what it holds, what its ledger explains, what `referral` says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Earned {
    /// Spot USDC held.
    pub held: Dec,
    /// What the ledger explains, net.
    pub moved: Dec,
    /// Whether every row read.
    pub exact: bool,
    /// What `referral` answered for the deployer's address.
    pub rewards: Option<Rewards>,
    /// What `referral` answered for the venue's builder code, where it is another address.
    pub coded: Option<Rewards>,
}

impl Earned {
    /// `me`'s spot USDC and its whole ledger.
    #[must_use]
    pub fn of(held: Dec, ledger: &[LedgerRow], me: Address) -> Earned {
        let (moved, exact) = explained(ledger, me);
        Earned { held, moved, exact, rewards: None, coded: None }
    }

    /// What the balance holds that no row explains: on a deployer's address, its fees. `None`
    /// when a row did not read: a hole in the ledger is not a total.
    #[must_use]
    pub fn fees(&self) -> Option<Estimate> {
        let amount = self.held.checked_sub(self.moved).filter(|_| self.exact)?;
        Some(Estimate { amount, basis: Basis::Ledger })
    }

    /// The builder code's pot where it was read, else the address's own.
    #[must_use]
    pub fn paid(&self) -> Option<&Rewards> {
        self.coded.as_ref().or(self.rewards.as_ref())
    }
}

#[cfg(test)]
#[path = "../tests/earned.rs"]
mod tests;
