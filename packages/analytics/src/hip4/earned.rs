//! What a deployer's address was paid, read off its own ledger: the USDC it holds less what the
//! rows of `userNonFundingLedgerUpdates` explain. On a deployer's address the rest is what its
//! markets paid it, which no endpoint reports, so it is an [`Estimate`] on the ledger's basis; a
//! row this build cannot read makes it unknown rather than wrong. Beside it, what `referral`
//! says its builder code was paid (hl-account's `Referral`, read as [`Rewards`]).
//!
//! The rows are read here as the venue sends them, every type by its fields, rather than as
//! hl-transfer's `LedgerUpdate`, which keeps the types it does not model (`rewardsClaim`,
//! `subAccountTransfer`, `vaultCreate`) without their amounts.

use hl_core::{Address, Dec};
use serde::Deserialize;

use crate::{
    builders::Rewards,
    estimate::{Basis, Estimate},
    math::add,
};

/// One row of `userNonFundingLedgerUpdates`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LedgerRow {
    /// Milliseconds.
    #[serde(default)]
    pub time: u64,
    #[serde(default)]
    pub hash: String,
    pub delta: LedgerDelta,
}

/// The fields of a row's `delta` that move USDC, as written: a figure that does not read makes
/// its row unread rather than zero.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerDelta {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(default)]
    pub usdc: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub fee: Option<String>,
    #[serde(default)]
    pub fee_token: Option<String>,
    #[serde(default)]
    pub to_perp: Option<bool>,
}

impl LedgerRow {
    /// The USDC this row moved into `me`'s account (out, below zero), less any USDC fee it
    /// charged `me`; `None` for a row this build does not read.
    #[must_use]
    pub fn moved(&self, me: Address) -> Option<Dec> {
        let delta = &self.delta;
        let figure = |said: &Option<String>| Dec::parse(said.as_deref()?).ok();
        let named =
            |said: &Option<String>| said.as_deref().is_some_and(|said| said.parse::<Address>().ok() == Some(me));
        let charged = match delta.fee_token.as_deref() == Some(QUOTE) && named(&delta.user) {
            true => delta.fee.as_deref().map_or(Some(Dec::ZERO), |fee| Dec::parse(fee).ok())?,
            false => Dec::ZERO,
        };
        let kind = delta.kind.as_str();
        let moved = if SIDED.contains(&kind) {
            let amount = match delta.token.as_deref() {
                Some(token) if token != QUOTE => Dec::ZERO,
                Some(_) => figure(&delta.amount)?,
                None => figure(&delta.usdc)?,
            };
            if named(&delta.destination) { amount } else { amount.negate() }
        } else if INWARD.contains(&kind) {
            figure(&delta.usdc)?
        } else if OUTWARD.contains(&kind) {
            figure(&delta.usdc)?.negate()
        } else if kind == "accountClassTransfer" {
            let amount = figure(&delta.usdc)?;
            if delta.to_perp == Some(true) { amount.negate() } else { amount }
        } else if STAKING.contains(&kind) {
            Dec::ZERO
        } else {
            return None;
        };
        moved.checked_sub(charged)
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

/// Rows with two ends: in for the end that is the address, out for the other.
const SIDED: [&str; 4] = ["send", "spotTransfer", "internalTransfer", "subAccountTransfer"];

const INWARD: [&str; 3] = ["deposit", "vaultWithdraw", "rewardsClaim"];

const OUTWARD: [&str; 3] = ["withdraw", "vaultDeposit", "vaultCreate"];

/// Rows that move no USDC but may charge a USDC fee.
const STAKING: [&str; 4] = ["cStakingTransfer", "delegate", "undelegate", "spotGenesis"];

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
