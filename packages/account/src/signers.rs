//! Who may sign what, as data per action kind: the account's own (main) key
//! only, or an agent too.
//!
//! - Every user-signed action needs the main key. The venue recovers the
//!   signer from the signature and acts for it, and an agent holds nothing,
//!   so an agent's approval, transfer or withdrawal would act for the agent.
//!   An agent therefore cannot approve a builder fee, nor another agent.
//! - An agent signs L1 actions for its master: orders, cancels, modifies,
//!   TWAPs, leverage and margin, `scheduleCancel`, `vaultTransfer`,
//!   `borrowLend`, `agentSetAbstraction`, `agentSendAsset`, `setReferrer`.
//! - `createSubAccount`, `subAccountTransfer`, `subAccountSpotTransfer` and
//!   HIP-4 splits and merges (`userOutcome`) take the main key too, as
//!   hip4-tui's docs/WIRE.md has it: an agent's would open or drain the
//!   wrong account.
//!
//! Recovery from a leaked agent key is replacement, since there is no revoke
//! ([`crate::agent`]).

use std::fmt;

/// An action, by the `type` the venue knows it as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionKind {
    Order,
    Cancel,
    CancelByCloid,
    Modify,
    BatchModify,
    TwapOrder,
    TwapCancel,
    UpdateLeverage,
    UpdateIsolatedMargin,
    ScheduleCancel,
    Noop,
    AgentSetAbstraction,
    AgentSendAsset,
    SetReferrer,
    CreateSubAccount,
    SubAccountTransfer,
    SubAccountSpotTransfer,
    VaultTransfer,
    BorrowLend,
    UserOutcome,
    UsdClassTransfer,
    UsdSend,
    SpotSend,
    SendAsset,
    Withdraw3,
    CDeposit,
    CWithdraw,
    TokenDelegate,
    ApproveAgent,
    ApproveBuilderFee,
    UserSetAbstraction,
}

/// Which key signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// The account's own wallet.
    Main,
    /// An agent the account approved.
    Agent,
}

/// Who may sign one kind of action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signers {
    /// The main key only, and why.
    Main(Why),
    /// The main key, or an agent for its master.
    MainOrAgent,
}

/// Why an action takes the main key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Why {
    /// User-signed: the signature recovers to the agent, which holds nothing.
    UserSigned,
    /// An agent's would open or drain the wrong account.
    WrongAccount,
}

impl Why {
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Why::UserSigned => "an agent's signature recovers to the agent, which holds nothing",
            Why::WrongAccount => "an agent's would open or drain the wrong account",
        }
    }
}

/// An action the key in hand may not sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind} is signed by the account's own key: {}", why.words())]
pub struct Refused {
    pub kind: ActionKind,
    pub why: Why,
}

/// Whether `key` may sign `kind`, refused before anything is signed.
pub fn may_sign(kind: ActionKind, key: Key) -> Result<(), Refused> {
    match (kind.signers(), key) {
        (Signers::Main(why), Key::Agent) => Err(Refused { kind, why }),
        _ => Ok(()),
    }
}

impl ActionKind {
    pub const ALL: [ActionKind; 31] = [
        ActionKind::Order,
        ActionKind::Cancel,
        ActionKind::CancelByCloid,
        ActionKind::Modify,
        ActionKind::BatchModify,
        ActionKind::TwapOrder,
        ActionKind::TwapCancel,
        ActionKind::UpdateLeverage,
        ActionKind::UpdateIsolatedMargin,
        ActionKind::ScheduleCancel,
        ActionKind::Noop,
        ActionKind::AgentSetAbstraction,
        ActionKind::AgentSendAsset,
        ActionKind::SetReferrer,
        ActionKind::CreateSubAccount,
        ActionKind::SubAccountTransfer,
        ActionKind::SubAccountSpotTransfer,
        ActionKind::VaultTransfer,
        ActionKind::BorrowLend,
        ActionKind::UserOutcome,
        ActionKind::UsdClassTransfer,
        ActionKind::UsdSend,
        ActionKind::SpotSend,
        ActionKind::SendAsset,
        ActionKind::Withdraw3,
        ActionKind::CDeposit,
        ActionKind::CWithdraw,
        ActionKind::TokenDelegate,
        ActionKind::ApproveAgent,
        ActionKind::ApproveBuilderFee,
        ActionKind::UserSetAbstraction,
    ];

    /// The action's `type`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            ActionKind::Order => "order",
            ActionKind::Cancel => "cancel",
            ActionKind::CancelByCloid => "cancelByCloid",
            ActionKind::Modify => "modify",
            ActionKind::BatchModify => "batchModify",
            ActionKind::TwapOrder => "twapOrder",
            ActionKind::TwapCancel => "twapCancel",
            ActionKind::UpdateLeverage => "updateLeverage",
            ActionKind::UpdateIsolatedMargin => "updateIsolatedMargin",
            ActionKind::ScheduleCancel => "scheduleCancel",
            ActionKind::Noop => "noop",
            ActionKind::AgentSetAbstraction => "agentSetAbstraction",
            ActionKind::AgentSendAsset => "agentSendAsset",
            ActionKind::SetReferrer => "setReferrer",
            ActionKind::CreateSubAccount => "createSubAccount",
            ActionKind::SubAccountTransfer => "subAccountTransfer",
            ActionKind::SubAccountSpotTransfer => "subAccountSpotTransfer",
            ActionKind::VaultTransfer => "vaultTransfer",
            ActionKind::BorrowLend => "borrowLend",
            ActionKind::UserOutcome => "userOutcome",
            ActionKind::UsdClassTransfer => "usdClassTransfer",
            ActionKind::UsdSend => "usdSend",
            ActionKind::SpotSend => "spotSend",
            ActionKind::SendAsset => "sendAsset",
            ActionKind::Withdraw3 => "withdraw3",
            ActionKind::CDeposit => "cDeposit",
            ActionKind::CWithdraw => "cWithdraw",
            ActionKind::TokenDelegate => "tokenDelegate",
            ActionKind::ApproveAgent => "approveAgent",
            ActionKind::ApproveBuilderFee => "approveBuilderFee",
            ActionKind::UserSetAbstraction => "userSetAbstraction",
        }
    }

    /// From the action's `type`, spelled as the venue spells it.
    #[must_use]
    pub fn from_wire(text: &str) -> Option<ActionKind> {
        ActionKind::ALL.into_iter().find(|kind| kind.wire() == text)
    }

    /// EIP-712 typed data the account's own wallet signs, rather than an L1
    /// action through the phantom agent.
    #[must_use]
    pub const fn is_user_signed(self) -> bool {
        matches!(
            self,
            ActionKind::UsdClassTransfer
                | ActionKind::UsdSend
                | ActionKind::SpotSend
                | ActionKind::SendAsset
                | ActionKind::Withdraw3
                | ActionKind::CDeposit
                | ActionKind::CWithdraw
                | ActionKind::TokenDelegate
                | ActionKind::ApproveAgent
                | ActionKind::ApproveBuilderFee
                | ActionKind::UserSetAbstraction
        )
    }

    #[must_use]
    pub const fn signers(self) -> Signers {
        if self.is_user_signed() {
            return Signers::Main(Why::UserSigned);
        }
        match self {
            ActionKind::CreateSubAccount
            | ActionKind::SubAccountTransfer
            | ActionKind::SubAccountSpotTransfer
            | ActionKind::UserOutcome => Signers::Main(Why::WrongAccount),
            _ => Signers::MainOrAgent,
        }
    }
}

impl fmt::Display for ActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

#[cfg(test)]
#[path = "tests/signers.rs"]
mod tests;
