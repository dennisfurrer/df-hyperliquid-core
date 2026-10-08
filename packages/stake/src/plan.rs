//! From a staking move's typed values to exactly what is signed: the action,
//! its nonce, the key it needs, the balance that should change once it lands,
//! and when what it moved is free again. hip4-tui's `tui/src/moves/plan.rs`,
//! without the form.

use hl_core::{Address, BadUnits, Dec, Network};

use crate::{
    actions::{CDeposit, CWithdraw, TokenDelegate},
    reads::{Delegation, DelegatorSummary, MAX_PENDING_WITHDRAWALS, locked_until, unstaking_ms},
};

/// Which key may sign an action (hip4-tui docs/WIRE.md, "Who may sign what").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Only the account's own key: every user-signed action. An agent's
    /// signature recovers to the agent, which holds nothing.
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
        "cDeposit" | "cWithdraw" | "tokenDelegate" => Some(Key::Main),
        _ => None,
    }
}

/// A figure a read back checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Balance {
    /// The staking balance free to delegate or withdraw:
    /// `delegatorSummary`'s `undelegated`.
    Undelegated,
    /// What is delegated to one validator: its `delegations` row.
    Delegated(Address),
    /// On its way to spot: `delegatorSummary`'s `totalPendingWithdrawal`.
    Pending,
}

/// How the figure moves once the action lands: by exactly the amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up(Dec),
    Down(Dec),
}

/// The balance an action changes, and what reading it back should show.
///
/// Every move here is read back on the staking balance: each one passes
/// through it, and rewards do not (they are added to the delegations), so it
/// moves by exactly the amount. The other side moves too: spot HYPE for a
/// deposit, the delegation for a delegate, the queue for a withdrawal.
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
            Change::Up(amount) => before.checked_add(amount) == Some(after),
            Change::Down(amount) => before.checked_sub(amount) == Some(after),
        }
    }
}

/// Exactly what is signed, and what to check once it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan<A> {
    pub network: Network,
    pub action: A,
    /// What it is signed with; the action carries it too.
    pub nonce: u64,
    pub key: Key,
    pub effect: Effect,
    /// When what it moved is free again, milliseconds: a delegation's lock
    /// ends, or a withdrawal reaches spot.
    pub free_at: Option<u64>,
}

/// Why a move is not signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("{amount} HYPE is more than the {held} there is")]
    MoreThan { amount: Dec, held: Dec },
    #[error("nothing is delegated to {0}")]
    NoDelegation(Address),
    #[error("that delegation is locked until {until} ms")]
    Locked { until: u64 },
    #[error("{MAX_PENDING_WITHDRAWALS} withdrawals already wait in the queue, the most there may be")]
    QueueFull,
    #[error(transparent)]
    Units(#[from] BadUnits),
}

/// Everything outside the amounts that a plan is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    pub network: Network,
    pub nonce: u64,
    /// Now, milliseconds: a lock or a queue is measured from it.
    pub now_ms: u64,
}

impl Ctx {
    const fn plan<A>(&self, action: A, key: Key, effect: Effect, free_at: Option<u64>) -> Plan<A> {
        Plan { network: self.network, action, nonce: self.nonce, key, effect, free_at }
    }

    /// `cDeposit`: HYPE from spot into the staking balance.
    pub fn stake(&self, hype: Dec) -> Result<Plan<CDeposit>, Refused> {
        let action = CDeposit::new(hype, self.nonce)?;
        let effect = Effect { balance: Balance::Undelegated, change: Change::Up(hype) };
        Ok(self.plan(action, CDeposit::KEY, effect, None))
    }

    /// `cWithdraw`: HYPE from the staking balance toward spot, through the
    /// network's queue, which holds at most five at once.
    pub fn unstake(&self, hype: Dec, summary: &DelegatorSummary) -> Result<Plan<CWithdraw>, Refused> {
        let action = CWithdraw::new(hype, self.nonce)?;
        at_most(hype, summary.undelegated)?;
        if summary.n_pending_withdrawals >= MAX_PENDING_WITHDRAWALS {
            return Err(Refused::QueueFull);
        }
        let effect = Effect { balance: Balance::Undelegated, change: Change::Down(hype) };
        let ready = self.now_ms.saturating_add(unstaking_ms(self.network));
        Ok(self.plan(action, CWithdraw::KEY, effect, Some(ready)))
    }

    /// `tokenDelegate`: the staking balance to `validator`, locked for the
    /// network's delegation lock.
    pub fn delegate(
        &self,
        validator: Address,
        hype: Dec,
        summary: &DelegatorSummary,
    ) -> Result<Plan<TokenDelegate>, Refused> {
        let action = TokenDelegate::delegate(validator, hype, self.nonce)?;
        at_most(hype, summary.undelegated)?;
        let effect = Effect { balance: Balance::Undelegated, change: Change::Down(hype) };
        Ok(self.plan(action, TokenDelegate::KEY, effect, Some(locked_until(self.network, self.now_ms))))
    }

    /// `tokenDelegate` back: from `validator` to the staking balance, once
    /// its lock has passed.
    pub fn undelegate(
        &self,
        validator: Address,
        hype: Dec,
        delegations: &[Delegation],
    ) -> Result<Plan<TokenDelegate>, Refused> {
        let action = TokenDelegate::undelegate(validator, hype, self.nonce)?;
        let held = delegations
            .iter()
            .find(|delegation| delegation.validator == validator)
            .ok_or(Refused::NoDelegation(validator))?;
        if held.is_locked(self.now_ms) {
            return Err(Refused::Locked { until: held.locked_until_timestamp });
        }
        at_most(hype, held.amount)?;
        let effect = Effect { balance: Balance::Undelegated, change: Change::Up(hype) };
        Ok(self.plan(action, TokenDelegate::KEY, effect, None))
    }
}

fn at_most(amount: Dec, held: Dec) -> Result<(), Refused> {
    match amount > held {
        true => Err(Refused::MoreThan { amount, held }),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
