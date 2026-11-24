//! How an account keeps its balances, from its mode: each perp dex and spot apart, USDC drawn on the
//! main dex, or one balance in spot. It decides which balance pays for an order, where USDC is read
//! back, and how a spot token leaves the account.

use crate::account::holdings::spot::SpotState;
use crate::account::settings::mode::Abstraction;

/// How an account keeps its balances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Keeping {
    /// Standard (`default`, `disabled`): each perp dex and spot keep their own.
    Apart,
    /// DEX abstraction: as standard, but every USDC dex draws on the main dex.
    Native,
    /// Unified: one balance per asset, held in spot. Refused `spotSend`.
    Unified,
    /// Portfolio margin: unified, and an order may borrow.
    Portfolio,
    /// Not read yet, or a mode this build does not know. Its balances read as standard; a spot
    /// token leaves by `sendAsset`, which every kind of account takes.
    Unknown,
}

impl Keeping {
    /// From the mode read, or, with none read, from the spot answer's own portfolio-margin flag.
    #[must_use]
    pub fn of(mode: Option<&Abstraction>, spot: Option<&SpotState>) -> Keeping {
        match mode {
            Some(Abstraction::UnifiedAccount) => Keeping::Unified,
            Some(Abstraction::PortfolioMargin) => Keeping::Portfolio,
            Some(Abstraction::DexAbstraction) => Keeping::Native,
            Some(Abstraction::Default | Abstraction::Disabled) => Keeping::Apart,
            Some(Abstraction::Unknown(_)) => Keeping::Unknown,
            None if spot.is_some_and(|spot| spot.portfolio_margin_enabled == Some(true)) => Keeping::Portfolio,
            None => Keeping::Unknown,
        }
    }

    /// Whether perps and spot draw on one balance.
    #[must_use]
    pub const fn pools(self) -> bool {
        matches!(self, Keeping::Unified | Keeping::Portfolio)
    }
}

#[cfg(test)]
#[path = "tests/keeping.rs"]
mod tests;
