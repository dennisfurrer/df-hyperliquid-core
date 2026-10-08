//! A figure no endpoint reports, worked out from what one does. What a HIP-4 deployer earns is
//! one: the venue bills the deployer's cut into each outcome fill's fee and reports what a
//! deployer was paid nowhere (hip4-tui's D11), so every such figure here is an [`Estimate`] and
//! says how it was made.

use hl_core::Dec;

/// An estimated amount, in the quote, and how it was estimated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    pub amount: Dec,
    pub basis: Basis,
}

/// How an [`Estimate`] was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// What an address holds that no row of its ledger explains: on a deployer's address, what
    /// its markets paid it, and anything else the venue credits without a row.
    Ledger,
    /// The fee model (D11) over `fills` fills: the deployer's part of what they paid the venue,
    /// `scale / (scale + max(scale, 1))` of it.
    Fills { fills: usize },
}
