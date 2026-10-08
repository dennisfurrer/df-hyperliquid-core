//! From a vault move's typed values to exactly what is signed: the action,
//! its nonce, the key it needs, the equity that should change once it lands,
//! and when a deposit unlocks. hip4-tui's `tui/src/moves/plan.rs`, without the
//! form.

use hl_core::{Address, BadUnits, Dec, Network, cut_units};

use crate::{
    reads::{VaultDetails, VaultEquity},
    transfer::{USD_DECIMALS, VaultTransfer, lockup_ms},
};

/// Which key may sign an action (hip4-tui docs/WIRE.md, "Who may sign what").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Only the account's own key.
    Main,
    /// An agent the account approved, or the account's own key.
    Either,
}

impl Key {
    /// Whether an agent's signature is taken.
    #[must_use]
    pub const fn takes_agent(self) -> bool {
        matches!(self, Key::Either)
    }
}

/// The key each of this crate's actions needs, by its wire `type`.
#[must_use]
pub fn key(kind: &str) -> Option<Key> {
    match kind {
        "vaultTransfer" => Some(Key::Either),
        _ => None,
    }
}

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
    Rises(Dec),
    Falls(Dec),
}

/// The equity an action changes, and what reading it back should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    pub balance: Balance,
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
    pub network: Network,
    pub action: A,
    pub nonce: u64,
    pub key: Key,
    pub effect: Effect,
    /// When a deposit may first be withdrawn, milliseconds.
    pub free_at: Option<u64>,
}

/// Why a move is not signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("{0} takes no deposits")]
    NoDeposits(Address),
    #[error("locked until {until} ms")]
    Locked { until: u64 },
    #[error("{amount} USDC is more than the {held} there is")]
    MoreThan { amount: Dec, held: Dec },
    #[error(transparent)]
    Units(#[from] BadUnits),
}

/// Everything outside the amounts that a plan is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    pub network: Network,
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
        let usd = cut_units(equity.equity, USD_DECIMALS)?;
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
