//! How an account keeps its balances, from `userAbstraction`'s word. It
//! decides how a spot token leaves the account (the venue refuses `spotSend`
//! to a pooled one) and where its USDC is read back. hip4-tui's
//! `tui/src/moves/usdc.rs`, after the venue's account abstraction docs.

/// Where an account's balances sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keeping {
    /// Standard (`disabled`, `default`, and the discontinued
    /// `dexAbstraction`): each perp dex and spot hold their own.
    Apart,
    /// Unified (`unifiedAccount`) or portfolio margin (`portfolioMargin`): one
    /// balance a token, in spot. Such an account is refused `spotSend`.
    Pooled,
    /// Not read yet, or a word this build does not know.
    Unknown,
}

impl Keeping {
    /// From `userAbstraction`'s answer.
    #[must_use]
    pub fn of(mode: &str) -> Keeping {
        match mode {
            "unifiedAccount" | "portfolioMargin" => Keeping::Pooled,
            "disabled" | "default" | "dexAbstraction" => Keeping::Apart,
            _ => Keeping::Unknown,
        }
    }

    /// Whether perps and spot draw on the one balance.
    #[must_use]
    pub const fn pools(self) -> bool {
        matches!(self, Keeping::Pooled)
    }
}
