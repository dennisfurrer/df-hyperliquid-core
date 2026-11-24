//! An account's mode: how its balances are kept, as `userAbstraction` answers, the read, and
//! changing it with `userSetAbstraction` (the account's own wallet) or `agentSetAbstraction` (an
//! agent, for its master). Unified back to standard is refused.

use std::fmt;

use serde::{Deserialize, Deserializer, de};

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::Address;
use crate::utils::sign::{Field, L1Action, Map, NonceKey, UserAction, UserSigned};

/// `{"type":"userAbstraction","user":..}`.
#[must_use]
pub fn abstraction_request(user: Address) -> String {
    user_request("userAbstraction", user)
}

/// Weighs 20.
pub async fn abstraction(transport: &impl Transport, user: Address) -> Result<Abstraction, ApiError> {
    info(transport, abstraction_request(user)).await
}

/// How an account's balances are kept, as `userAbstraction` answers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Abstraction {
    /// What a new account answers, never set. Standard: perp and spot apart.
    Default,
    /// Standard, set: each perp dex and spot keep their own.
    Disabled,
    /// One balance per asset, in spot.
    UnifiedAccount,
    /// Unified, and an order may borrow against HYPE and BTC.
    PortfolioMargin,
    /// Discontinued: USDC sits with the main dex's perps, and every USDC dex
    /// draws on it; everything else as standard.
    DexAbstraction,
    /// A mode this build does not know, kept as spelled. Treated as standard.
    Unknown(String),
}

impl Abstraction {
    /// From the venue's spelling: `unifiedAccount`. A mode this build does not know is kept as
    /// [`Abstraction::Unknown`].
    #[must_use]
    pub fn parse(text: &str) -> Abstraction {
        match text {
            "default" => Abstraction::Default,
            "disabled" => Abstraction::Disabled,
            "unifiedAccount" => Abstraction::UnifiedAccount,
            "portfolioMargin" => Abstraction::PortfolioMargin,
            "dexAbstraction" => Abstraction::DexAbstraction,
            other => Abstraction::Unknown(other.to_owned()),
        }
    }

    /// As the venue spells it: `unifiedAccount`.
    #[must_use]
    pub fn wire(&self) -> &str {
        match self {
            Abstraction::Default => "default",
            Abstraction::Disabled => "disabled",
            Abstraction::UnifiedAccount => "unifiedAccount",
            Abstraction::PortfolioMargin => "portfolioMargin",
            Abstraction::DexAbstraction => "dexAbstraction",
            Abstraction::Unknown(other) => other,
        }
    }

    /// One balance, in spot: a unified or portfolio-margin account, which also
    /// refuses `spotSend`.
    #[must_use]
    pub const fn is_pooled(&self) -> bool {
        matches!(self, Abstraction::UnifiedAccount | Abstraction::PortfolioMargin)
    }

    /// Separate ledgers: each perp dex and spot keep their own.
    #[must_use]
    pub const fn is_standard(&self) -> bool {
        !self.is_pooled()
    }
}

impl fmt::Display for Abstraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// From the bare JSON string the venue answers.
impl<'de> Deserialize<'de> for Abstraction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = Abstraction;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an account mode as a string")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Abstraction, E> {
                Ok(Abstraction::parse(text))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

/// A mode an account can be set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AbstractionMode {
    /// Standard: perp and spot apart.
    Disabled,
    /// One balance, in spot.
    UnifiedAccount,
    /// Unified, and an order may borrow against HYPE and BTC.
    PortfolioMargin,
}

impl AbstractionMode {
    /// Every mode, standard first.
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
    /// Sets `user`'s mode to `mode`, signed at `nonce`.
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
    /// Sets the master's mode to `mode`.
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

#[cfg(test)]
#[path = "tests/mode.rs"]
mod tests;
