//! Changing an account's mode: `userSetAbstraction` (the account's own
//! wallet) or `agentSetAbstraction` (an agent, for its master).
//!
//! **Unified back to standard is refused** by the venue; a unified account
//! stays pooled.

use hl_core::{Address, Field, L1Action, Map, NonceKey, UserAction, UserSigned};

use crate::state::Abstraction;

/// A mode an account can be set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbstractionMode {
    /// Standard: perp and spot apart.
    Disabled,
    /// One balance, in spot.
    UnifiedAccount,
    PortfolioMargin,
}

impl AbstractionMode {
    pub const ALL: [AbstractionMode; 3] =
        [AbstractionMode::Disabled, AbstractionMode::UnifiedAccount, AbstractionMode::PortfolioMargin];

    /// As `userSetAbstraction` and `userAbstraction` spell it.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            AbstractionMode::Disabled => "disabled",
            AbstractionMode::UnifiedAccount => "unifiedAccount",
            AbstractionMode::PortfolioMargin => "portfolioMargin",
        }
    }

    /// As `agentSetAbstraction` spells it.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            AbstractionMode::Disabled => "i",
            AbstractionMode::UnifiedAccount => "u",
            AbstractionMode::PortfolioMargin => "p",
        }
    }

    /// From `userSetAbstraction`'s spelling. `default`, a mode never set, is
    /// none an account can be set to.
    #[must_use]
    pub fn from_wire(text: &str) -> Option<AbstractionMode> {
        AbstractionMode::ALL.into_iter().find(|mode| mode.wire() == text)
    }

    /// From `agentSetAbstraction`'s one letter.
    #[must_use]
    pub fn from_code(text: &str) -> Option<AbstractionMode> {
        AbstractionMode::ALL.into_iter().find(|mode| mode.code() == text)
    }

    /// Whether the venue refuses moving an account in `current` to this
    /// mode: a unified account back to standard.
    #[must_use]
    pub const fn is_refused_from(self, current: &Abstraction) -> bool {
        matches!((current, self), (Abstraction::UnifiedAccount, AbstractionMode::Disabled))
    }
}

/// `userSetAbstraction`, signed by the account's own wallet. `user` is the
/// account whose mode changes. Unified back to standard is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSetAbstraction {
    user: Address,
    mode: AbstractionMode,
    nonce: u64,
}

impl UserSetAbstraction {
    #[must_use]
    pub const fn new(user: Address, mode: AbstractionMode, nonce: u64) -> UserSetAbstraction {
        UserSetAbstraction { user, mode, nonce }
    }
}

impl UserAction for UserSetAbstraction {
    fn user_signed(&self) -> UserSigned<'_> {
        UserSigned::new("userSetAbstraction", "HyperliquidTransaction:UserSetAbstraction", NonceKey::Nonce, self.nonce)
            .field("user", Field::Address(self.user))
            .field("abstraction", Field::String(self.mode.wire()))
    }
}

/// `agentSetAbstraction`: `{type, abstraction}` with the one-letter code, an
/// L1 action an agent signs for its master. Unified back to standard is
/// refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentSetAbstraction {
    mode: AbstractionMode,
}

impl AgentSetAbstraction {
    #[must_use]
    pub const fn new(mode: AbstractionMode) -> AgentSetAbstraction {
        AgentSetAbstraction { mode }
    }
}

impl L1Action for AgentSetAbstraction {
    fn action(&self) -> Map {
        Map::new().put("type", "agentSetAbstraction").put("abstraction", self.mode.code())
    }
}
