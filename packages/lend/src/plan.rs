//! From a lending move's typed values to exactly what is signed: the action,
//! its nonce, the key it needs, and the position that should change once it
//! lands. hip4-tui's `tui/src/moves/plan.rs`, without the form. A token is its
//! spot index from the caller's `spotMeta`.

use hl_core::{ActionKind, Dec, Network};

use crate::{
    action::{BadLend, BorrowLend},
    state::{ReserveState, UserState, reserve},
};

/// Who may sign an action (hip4-tui docs/WIRE.md, "Who may sign what"):
/// hl-core's [`Signers`](hl_core::Signers), the rule hl-account checks a key
/// against. `Main` is the account's own key only; `Either` takes an agent the
/// account approved too.
pub use hl_core::Signers as Key;

/// This crate's actions.
const KINDS: [ActionKind; 1] = [ActionKind::BorrowLend];

/// The key each of this crate's actions needs, by its wire `type`.
#[must_use]
pub fn key(kind: &str) -> Option<Key> {
    ActionKind::from_wire(kind).filter(|kind| KINDS.contains(kind)).map(ActionKind::signers)
}

/// A figure a read back checks: a position's `value` in
/// `borrowLendUserState`, by token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Balance {
    Supplied(u32),
    Borrowed(u32),
}

/// How the position moves once the action lands. Interest moves it too, so a
/// read back shows the direction, by about the amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Rises(Dec),
    Falls(Dec),
    /// To nothing: all of it withdrawn or repaid.
    Empties,
}

/// The position an action changes, and what reading it back should show.
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
            Change::Empties => after.is_zero(),
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
}

/// Why a move is not signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("token {0} has no lending reserve")]
    NoReserve(u32),
    #[error("{amount} is more than the {held} there is")]
    MoreThan { amount: Dec, held: Dec },
    #[error("nothing of token {0} is there to take back")]
    Nothing(u32),
    #[error(transparent)]
    Lend(#[from] BadLend),
}

/// Everything outside the amounts that a plan is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    pub network: Network,
    pub nonce: u64,
}

impl Ctx {
    const fn plan(&self, action: BorrowLend, effect: Effect) -> Plan<BorrowLend> {
        Plan { network: self.network, action, nonce: self.nonce, key: BorrowLend::KEY, effect }
    }

    /// Supplies `amount` of `token`, already cut to its `weiDecimals`.
    pub fn supply(
        &self,
        token: u32,
        amount: Dec,
        reserves: &[(u32, ReserveState)],
    ) -> Result<Plan<BorrowLend>, Refused> {
        let action = BorrowLend::supply(token, amount)?;
        reserve(reserves, token).ok_or(Refused::NoReserve(token))?;
        Ok(self.plan(action, Effect { balance: Balance::Supplied(token), change: Change::Rises(amount) }))
    }

    /// Takes back `amount` of what is supplied, or with `None` all of it.
    pub fn withdraw(&self, token: u32, amount: Option<Dec>, state: &UserState) -> Result<Plan<BorrowLend>, Refused> {
        let held = state.supplied(token);
        let (action, change) = back(token, amount, held, BorrowLend::withdraw, BorrowLend::withdraw_all)?;
        Ok(self.plan(action, Effect { balance: Balance::Supplied(token), change }))
    }

    /// Borrows `amount` of `token`, at most what its reserve can lend.
    pub fn borrow(
        &self,
        token: u32,
        amount: Dec,
        reserves: &[(u32, ReserveState)],
    ) -> Result<Plan<BorrowLend>, Refused> {
        let action = BorrowLend::borrow(token, amount)?;
        let lendable = reserve(reserves, token).ok_or(Refused::NoReserve(token))?.lendable();
        if amount > lendable {
            return Err(Refused::MoreThan { amount, held: lendable });
        }
        Ok(self.plan(action, Effect { balance: Balance::Borrowed(token), change: Change::Rises(amount) }))
    }

    /// Repays `amount` of what is owed, or with `None` all of it, interest in.
    pub fn repay(&self, token: u32, amount: Option<Dec>, state: &UserState) -> Result<Plan<BorrowLend>, Refused> {
        let held = state.borrowed(token);
        let (action, change) = back(token, amount, held, BorrowLend::repay, BorrowLend::repay_all)?;
        Ok(self.plan(action, Effect { balance: Balance::Borrowed(token), change }))
    }
}

/// A withdraw or repay of `amount` against `held`, or of all of it.
fn back(
    token: u32,
    amount: Option<Dec>,
    held: Dec,
    some: fn(u32, Dec) -> Result<BorrowLend, BadLend>,
    all: fn(u32) -> BorrowLend,
) -> Result<(BorrowLend, Change), Refused> {
    if !held.is_positive() {
        return Err(Refused::Nothing(token));
    }
    match amount {
        None => Ok((all(token), Change::Empties)),
        Some(amount) => {
            let action = some(token, amount)?;
            match amount > held {
                true => Err(Refused::MoreThan { amount, held }),
                false => Ok((action, Change::Falls(amount))),
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
