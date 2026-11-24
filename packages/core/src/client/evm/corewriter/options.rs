//! The small enums some actions carry as a `uint8`: a limit order's time in
//! force, a finalize variant, a lending operation, an account mode, a HIP-4
//! operation. A code the docs do not name is kept as it came.

/// A limit order's time in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tif {
    /// Add liquidity only (post only), code 1.
    Alo,
    /// Good til cancelled, code 2.
    Gtc,
    /// Immediate or cancel, code 3.
    Ioc,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl Tif {
    /// The `encodedTif` byte: 1, 2 or 3, or the kept code for `Other`.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Tif::Alo => 1,
            Tif::Gtc => 2,
            Tif::Ioc => 3,
            Tif::Other(code) => code,
        }
    }

    /// The time in force for an `encodedTif` byte; an unnamed code becomes `Other`.
    #[must_use]
    pub const fn from_code(code: u8) -> Tif {
        match code {
            1 => Tif::Alo,
            2 => Tif::Gtc,
            3 => Tif::Ioc,
            other => Tif::Other(other),
        }
    }
}

/// How the deployer of an ERC-20 proves it to finalize the link (action 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FinalizeVariant {
    /// The deploying address, with the nonce it deployed at.
    Create,
    /// The address in the contract's first storage slot.
    FirstStorageSlot,
    /// The address in the slot at `keccak256("HyperCore deployer")`.
    CustomStorageSlot,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl FinalizeVariant {
    /// The `encodedFinalizeEvmContractVariant` byte: 1, 2 or 3, or the kept code for `Other`.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            FinalizeVariant::Create => 1,
            FinalizeVariant::FirstStorageSlot => 2,
            FinalizeVariant::CustomStorageSlot => 3,
            FinalizeVariant::Other(code) => code,
        }
    }

    /// The variant for an `encodedFinalizeEvmContractVariant` byte; an unnamed code becomes `Other`.
    #[must_use]
    pub const fn from_code(code: u8) -> FinalizeVariant {
        match code {
            1 => FinalizeVariant::Create,
            2 => FinalizeVariant::FirstStorageSlot,
            3 => FinalizeVariant::CustomStorageSlot,
            other => FinalizeVariant::Other(other),
        }
    }
}

/// A lending operation (action 15). The docs give supply and withdraw; on
/// chain (observed 2026-08-21), 2 routes as borrow and 3 as repay, and 4, 5
/// and 255 are discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BorrowLendOp {
    /// Supply the token to lending, code 0.
    Supply,
    /// Withdraw the supplied token, code 1.
    Withdraw,
    /// Undocumented.
    Borrow,
    /// Undocumented.
    Repay,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl BorrowLendOp {
    /// The `encodedOperation` byte: 0 to 3, or the kept code for `Other`.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            BorrowLendOp::Supply => 0,
            BorrowLendOp::Withdraw => 1,
            BorrowLendOp::Borrow => 2,
            BorrowLendOp::Repay => 3,
            BorrowLendOp::Other(code) => code,
        }
    }

    /// The operation for an `encodedOperation` byte; an unnamed code becomes `Other`.
    #[must_use]
    pub const fn from_code(code: u8) -> BorrowLendOp {
        match code {
            0 => BorrowLendOp::Supply,
            1 => BorrowLendOp::Withdraw,
            2 => BorrowLendOp::Borrow,
            3 => BorrowLendOp::Repay,
            other => BorrowLendOp::Other(other),
        }
    }
}

/// An account mode (action 16), for the master account or a subaccount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Abstraction {
    /// No account abstraction, code 1.
    Disabled,
    /// Unified account, code 2.
    UnifiedAccount,
    /// Portfolio margin, code 3.
    PortfolioMargin,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl Abstraction {
    /// The `abstraction` byte: 1, 2 or 3, or the kept code for `Other`.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Abstraction::Disabled => 1,
            Abstraction::UnifiedAccount => 2,
            Abstraction::PortfolioMargin => 3,
            Abstraction::Other(code) => code,
        }
    }

    /// The mode for an `abstraction` byte; an unnamed code becomes `Other`.
    #[must_use]
    pub const fn from_code(code: u8) -> Abstraction {
        match code {
            1 => Abstraction::Disabled,
            2 => Abstraction::UnifiedAccount,
            3 => Abstraction::PortfolioMargin,
            other => Abstraction::Other(other),
        }
    }
}

/// A HIP-4 operation (action 17). Unused fields must be zero, or HyperCore
/// drops the payload without a word: split and merge ignore `question`,
/// merge question ignores `outcome`, negate uses both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutcomeOp {
    /// Quote tokens into as many of each side of an outcome, code 0.
    SplitOutcome,
    /// One of each side of an outcome back into a quote token, code 1.
    MergeOutcome,
    /// A Yes of every outcome of a question back into a quote token, code 2.
    MergeQuestion,
    /// An outcome's No into a Yes of every other outcome of its question, code 3.
    NegateOutcome,
    /// A value the docs do not name, kept as it came.
    Other(u8),
}

impl OutcomeOp {
    /// The `encodedOperation` byte: 0 to 3, or the kept code for `Other`.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            OutcomeOp::SplitOutcome => 0,
            OutcomeOp::MergeOutcome => 1,
            OutcomeOp::MergeQuestion => 2,
            OutcomeOp::NegateOutcome => 3,
            OutcomeOp::Other(code) => code,
        }
    }

    /// The operation for an `encodedOperation` byte; an unnamed code becomes `Other`.
    #[must_use]
    pub const fn from_code(code: u8) -> OutcomeOp {
        match code {
            0 => OutcomeOp::SplitOutcome,
            1 => OutcomeOp::MergeOutcome,
            2 => OutcomeOp::MergeQuestion,
            3 => OutcomeOp::NegateOutcome,
            other => OutcomeOp::Other(other),
        }
    }
}

#[cfg(test)]
#[path = "tests/options.rs"]
mod tests;
