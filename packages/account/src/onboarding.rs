//! Onboarding as data: what is left before an account can trade, computed
//! from live state every time and never remembered (atlas's `Onboarding`).
//!
//! 1. Deposit: done once the account holds anything, perps or spot. The
//!    venue refuses every other action before a deposit ("Must deposit
//!    before performing actions"), so the steps below wait for it. Running it
//!    is a USDC transfer to the bridge on Arbitrum (Arbitrum Sepolia on
//!    testnet), which the wallet sends, not the venue; a subaccount is funded
//!    by its master instead.
//! 2. Approve an agent: done while `extraAgents` lists it unexpired, or,
//!    for the unnamed agent, which it does not list, while `userRole` says
//!    the agent signs for this account.
//! 3. Approve a builder fee: done once `maxBuilderFee` covers the fee. Only
//!    when a builder is passed in; a subaccount approves nothing.
//! 4. Set the account mode: done once `userAbstraction` answers it. Unified
//!    back to standard is refused by the venue.
//!
//! Nothing app-specific is here: the agent, the builder and its fee, and the
//! mode are what the caller passes in.

use hl_core::{Address, BuilderFee, Dec, Network};

use crate::{
    agent::{Agent, Role},
    mode::AbstractionMode,
    signers::ActionKind,
    state::{Abstraction, PerpState, SpotState},
    sub_account::AccountRef,
};

/// The bridge credits nothing under 5 USDC.
pub const MIN_DEPOSIT: Dec = Dec::from_int(5);

/// What the caller wants of the account. Each `None` leaves its step out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wants {
    /// The agent that will sign, for the owner or for the subaccount.
    pub agent: Option<Address>,
    /// A builder and the fee it charges, in tenths of a basis point.
    pub builder: Option<(Address, BuilderFee)>,
    pub mode: Option<AbstractionMode>,
}

/// What the venue said, read at the account's own address, except `agents`
/// and the agent's role, which are the signing account's.
#[derive(Debug, Clone, Copy)]
pub struct Seen<'a> {
    /// The main dex.
    pub perp: &'a PerpState,
    pub spot: &'a SpotState,
    pub mode: &'a Abstraction,
    /// `extraAgents` of the account that signs.
    pub agents: &'a [Agent],
    /// `userRole` of the wanted agent, where it was read.
    pub agent_role: Option<&'a Role>,
    /// `maxBuilderFee` for the wanted builder, where it was read.
    pub approved: Option<BuilderFee>,
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepKind {
    /// USDC to the bridge on `chain_id`, at least `min`.
    Deposit {
        chain_id: u64,
        min: Dec,
    },
    /// `subAccountTransfer` from the master.
    FundFromMaster,
    ApproveAgent {
        agent: Address,
    },
    ApproveBuilderFee {
        builder: Address,
        fee: BuilderFee,
    },
    SetMode {
        mode: AbstractionMode,
    },
}

impl StepKind {
    /// A name that stays the same: `deposit`, `approve-agent`, `builder-fee`,
    /// `account-mode`.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            StepKind::Deposit { .. } | StepKind::FundFromMaster => "deposit",
            StepKind::ApproveAgent { .. } => "approve-agent",
            StepKind::ApproveBuilderFee { .. } => "builder-fee",
            StepKind::SetMode { .. } => "account-mode",
        }
    }

    /// The venue's action that does it; none for a deposit, which goes to
    /// the bridge. Setting the mode is `userSetAbstraction` with the main
    /// key, or `agentSetAbstraction` with an agent's.
    #[must_use]
    pub const fn action(&self) -> Option<ActionKind> {
        match self {
            StepKind::Deposit { .. } => None,
            StepKind::FundFromMaster => Some(ActionKind::SubAccountTransfer),
            StepKind::ApproveAgent { .. } => Some(ActionKind::ApproveAgent),
            StepKind::ApproveBuilderFee { .. } => Some(ActionKind::ApproveBuilderFee),
            StepKind::SetMode { .. } => Some(ActionKind::UserSetAbstraction),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Done,
    Needed,
    Blocked(Blocked),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// The venue refuses every action before a deposit.
    DepositFirst,
    /// The venue refuses it, in words.
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub kind: StepKind,
    pub status: Status,
}

/// Whether the account holds anything, perps or spot.
#[must_use]
pub fn deposited(perp: &PerpState, spot: &SpotState) -> bool {
    perp.margin_summary.account_value.is_positive() || spot.balances.iter().any(|row| row.total.is_positive())
}

/// The steps for `account` on `network` at `now` milliseconds, in the order
/// they are done.
#[must_use]
pub fn steps(account: &AccountRef, network: Network, seen: &Seen<'_>, wants: &Wants, now: u64) -> Vec<Step> {
    let funded = deposited(seen.perp, seen.spot);
    let after = |done: bool| match (done, funded) {
        (true, _) => Status::Done,
        (false, true) => Status::Needed,
        (false, false) => Status::Blocked(Blocked::DepositFirst),
    };
    let deposit = match account.is_sub() {
        true => StepKind::FundFromMaster,
        false => StepKind::Deposit { chain_id: network.arbitrum_chain_id(), min: MIN_DEPOSIT },
    };
    let mut steps = vec![Step { kind: deposit, status: if funded { Status::Done } else { Status::Needed } }];
    if let Some(agent) = wants.agent {
        let listed = seen.agents.iter().any(|known| known.address == agent && known.is_live(now));
        let signs = seen.agent_role.and_then(Role::agent_of) == Some(account.signs_for());
        steps.push(Step { kind: StepKind::ApproveAgent { agent }, status: after(listed || signs) });
    }
    if let Some((builder, fee)) = wants.builder.filter(|_| !account.is_sub()) {
        let covered = seen.approved.is_some_and(|approved| approved >= fee);
        steps.push(Step { kind: StepKind::ApproveBuilderFee { builder, fee }, status: after(covered) });
    }
    if let Some(mode) = wants.mode {
        let status = match (seen.mode.wire() == mode.wire(), mode.is_refused_from(seen.mode)) {
            (true, _) => Status::Done,
            (false, true) => {
                Status::Blocked(Blocked::Refused("the venue refuses a unified account back to standard".to_owned()))
            }
            (false, false) => after(false),
        };
        steps.push(Step { kind: StepKind::SetMode { mode }, status });
    }
    steps
}

/// The steps not done yet.
pub fn left(steps: &[Step]) -> impl Iterator<Item = &Step> {
    steps.iter().filter(|step| step.status != Status::Done)
}

#[cfg(test)]
#[path = "tests/onboarding.rs"]
mod tests;
