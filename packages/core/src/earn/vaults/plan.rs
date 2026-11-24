//! From a vault move's typed values to exactly what is signed: the action,
//! its nonce, the key it needs, the equity that should change once it lands,
//! and when a deposit unlocks.

use crate::account::Signers;
use crate::client::Network;
use crate::earn::vaults::reads::{VaultDetails, VaultEquity};
use crate::earn::vaults::transfer::{VaultTransfer, lockup_ms};
use crate::utils::{Address, BadUnits, Dec, USDC_DECIMALS, cut_units};

/// A figure a read back checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Balance {
    /// The account's equity in a vault: its `userVaultEquities` row. The perp
    /// USDC on the other side moves by exactly the amount.
    Vault(Address),
}

/// How the equity moves once the action lands. The vault's profit and loss
/// moves it too, so a read back shows the direction, by about the amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Rises by about this much USDC.
    Rises(Dec),
    /// Falls by about this much USDC.
    Falls(Dec),
}

/// The equity an action changes, and what reading it back should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    /// The equity that changes.
    pub balance: Balance,
    /// How it changes.
    pub change: Change,
}

impl Effect {
    /// Whether `after`, read back, shows the action landed against `before`.
    #[must_use]
    pub fn landed(&self, before: Dec, after: Dec) -> bool {
        match self.change {
            Change::Rises(_) => after > before,
            Change::Falls(_) => after < before,
        }
    }
}

/// Exactly what is signed, and what to check once it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan<A> {
    /// The network the action is signed for.
    pub network: Network,
    /// The action signed.
    pub action: A,
    /// The nonce it is signed with, in milliseconds.
    pub nonce: u64,
    /// Which keys may sign it.
    pub key: Signers,
    /// The equity that should change, to check once it lands.
    pub effect: Effect,
    /// When a deposit may first be withdrawn, milliseconds.
    pub free_at: Option<u64>,
}

/// Why a move is not signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The vault is closed or takes no deposits; holds its address.
    #[error("{0} takes no deposits")]
    NoDeposits(Address),
    /// The deposit is still in its lockup.
    #[error("locked until {until} ms")]
    Locked {
        /// When the lockup ends, milliseconds.
        until: u64,
    },
    /// An amount over the equity held.
    #[error("{amount} USDC is more than the {held} there is")]
    MoreThan {
        /// The USDC asked for.
        amount: Dec,
        /// The USDC equity there is.
        held: Dec,
    },
    /// The amount has more than six decimal places.
    #[error(transparent)]
    Units(#[from] BadUnits),
}

/// Everything outside the amounts that a plan is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    /// The network the action is signed for.
    pub network: Network,
    /// The nonce to sign with, in milliseconds.
    pub nonce: u64,
    /// Now, milliseconds: a lockup is measured from it.
    pub now_ms: u64,
}

impl Ctx {
    const fn plan(&self, action: VaultTransfer, change: Change, free_at: Option<u64>) -> Plan<VaultTransfer> {
        let effect = Effect { balance: Balance::Vault(action.vault()), change };
        Plan { network: self.network, action, nonce: self.nonce, key: VaultTransfer::KEY, effect, free_at }
    }

    /// USDC from the perp balance into `vault`, locked for its lockup.
    pub fn deposit(&self, vault: &VaultDetails, usdc: Dec) -> Result<Plan<VaultTransfer>, Refused> {
        let action = VaultTransfer::deposit(vault.vault_address, usdc)?;
        if !vault.takes_deposits() {
            return Err(Refused::NoDeposits(vault.vault_address));
        }
        let free_at = self.now_ms.saturating_add(lockup_ms(self.network, vault.vault_address));
        Ok(self.plan(action, Change::Rises(usdc), Some(free_at)))
    }

    /// `usdc` out of the vault `equity` is in, once its lockup has passed.
    pub fn withdraw(&self, equity: &VaultEquity, usdc: Dec) -> Result<Plan<VaultTransfer>, Refused> {
        let action = VaultTransfer::withdraw(equity.vault_address, usdc)?;
        unlocked(equity, self.now_ms)?;
        if usdc > equity.equity {
            return Err(Refused::MoreThan { amount: usdc, held: equity.equity });
        }
        Ok(self.plan(action, Change::Falls(usdc), None))
    }

    /// The whole equity, cut to millionths so it never asks for more.
    pub fn withdraw_all(&self, equity: &VaultEquity) -> Result<Plan<VaultTransfer>, Refused> {
        unlocked(equity, self.now_ms)?;
        let usd = cut_units(equity.equity, USDC_DECIMALS)?;
        let action = VaultTransfer::from_micros(equity.vault_address, false, usd);
        let amount = equity.withdrawable(self.now_ms);
        Ok(self.plan(action, Change::Falls(amount), None))
    }
}

fn unlocked(equity: &VaultEquity, now_ms: u64) -> Result<(), Refused> {
    match equity.is_locked(now_ms) {
        true => Err(Refused::Locked { until: equity.locked_until_timestamp }),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
