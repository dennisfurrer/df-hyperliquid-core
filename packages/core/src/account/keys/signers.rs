//! Who may sign what, per action kind: the account's own (main) key only, or an agent too.
//! User-signed actions, subaccount actions and HIP-4 splits and merges take the main key; an agent
//! signs the other L1 actions for its master.

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
    TrailingStop,
    CDeposit2,
    ClaimRewards,
    ReserveRequestWeight,
    Hip3LiquidatorTransfer,
    SendToEvmWithData,
    ConvertToMultiSigUser,
}

/// Which key signs: the one in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// The account's own wallet.
    Main,
    /// An agent the account approved.
    Agent,
}

/// Who may sign one kind of action. [`ActionKind::why`] says why an action
/// takes the main key alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signers {
    /// The account's own key only.
    Main,
    /// The account's own key, or an agent for its master.
    Either,
}

impl Signers {
    /// Whether an agent's signature is taken.
    #[must_use]
    pub const fn takes_agent(self) -> bool {
        matches!(self, Signers::Either)
    }
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
    match (kind.why(), key) {
        (Some(why), Key::Agent) => Err(Refused { kind, why }),
        _ => Ok(()),
    }
}

impl ActionKind {
    pub const ALL: [ActionKind; 38] = [
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
        ActionKind::TrailingStop,
        ActionKind::CDeposit2,
        ActionKind::ClaimRewards,
        ActionKind::ReserveRequestWeight,
        ActionKind::Hip3LiquidatorTransfer,
        ActionKind::SendToEvmWithData,
        ActionKind::ConvertToMultiSigUser,
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
            ActionKind::TrailingStop => "trailingStop",
            ActionKind::CDeposit2 => "cDeposit2",
            ActionKind::ClaimRewards => "claimRewards",
            ActionKind::ReserveRequestWeight => "reserveRequestWeight",
            ActionKind::Hip3LiquidatorTransfer => "hip3LiquidatorTransfer",
            ActionKind::SendToEvmWithData => "sendToEvmWithData",
            ActionKind::ConvertToMultiSigUser => "convertToMultiSigUser",
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
                | ActionKind::SendToEvmWithData
                | ActionKind::ConvertToMultiSigUser
        )
    }

    #[must_use]
    pub const fn signers(self) -> Signers {
        match self.why() {
            Some(_) => Signers::Main,
            None => Signers::Either,
        }
    }

    /// Why only the account's own key may sign it; `None` when an agent may.
    #[must_use]
    pub const fn why(self) -> Option<Why> {
        if self.is_user_signed() {
            return Some(Why::UserSigned);
        }
        match self {
            ActionKind::CreateSubAccount
            | ActionKind::SubAccountTransfer
            | ActionKind::SubAccountSpotTransfer
            | ActionKind::UserOutcome => Some(Why::WrongAccount),
            _ => None,
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
