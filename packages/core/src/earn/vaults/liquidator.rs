//! `hip3LiquidatorTransfer`: `{type, dex, ntl, isDeposit}`, keys in that order, as
//! `@nktkas/hyperliquid` 0.33.3 posts them. It moves a HIP-3 dex's quote token into or out of the
//! dex's backstop liquidator. `ntl` is in millionths of the quote token, whatever its own decimals,
//! and is a whole multiple of a thousand tokens. Only the principal comes back out, not the profit.

use crate::account::{ActionKind, Signers};
use crate::utils::sign::{L1Action, Map};
use crate::utils::{BadUnits, Dec, Round, exact_units};

/// What a deposit or withdrawal is a whole multiple of, in the quote token.
pub const LIQUIDATOR_STEP: Dec = Dec::from_int(1_000);

/// The places `ntl` counts the quote token in.
pub const LIQUIDATOR_DECIMALS: u32 = 6;

/// Why an amount is not one the backstop liquidator takes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadLiquidatorTransfer {
    #[error("the main dex has no backstop liquidator; name a HIP-3 dex")]
    NoDex,
    #[error("{0} is not a whole multiple of {LIQUIDATOR_STEP} of the quote token")]
    NotAStep(Dec),
    #[error(transparent)]
    Units(#[from] BadUnits),
}

/// Quote token into or out of a HIP-3 dex's backstop liquidator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hip3LiquidatorTransfer {
    dex: String,
    ntl: u64,
    is_deposit: bool,
}

impl Hip3LiquidatorTransfer {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::Hip3LiquidatorTransfer.signers();

    /// `amount` of the quote token into the liquidator of the HIP-3 dex `dex` (`xyz`).
    pub fn deposit(dex: &str, amount: Dec) -> Result<Hip3LiquidatorTransfer, BadLiquidatorTransfer> {
        Hip3LiquidatorTransfer::new(dex, amount, true)
    }

    /// `amount` of the quote token back out of the liquidator of the HIP-3 dex `dex`, up to the
    /// principal put in.
    pub fn withdraw(dex: &str, amount: Dec) -> Result<Hip3LiquidatorTransfer, BadLiquidatorTransfer> {
        Hip3LiquidatorTransfer::new(dex, amount, false)
    }

    fn new(dex: &str, amount: Dec, is_deposit: bool) -> Result<Hip3LiquidatorTransfer, BadLiquidatorTransfer> {
        if dex.is_empty() {
            return Err(BadLiquidatorTransfer::NoDex);
        }
        let whole_steps = amount.checked_div(LIQUIDATOR_STEP, 0, Round::Down);
        if !amount.is_positive() || whole_steps.and_then(|steps| steps.checked_mul(LIQUIDATOR_STEP)) != Some(amount) {
            return Err(BadLiquidatorTransfer::NotAStep(amount));
        }
        Ok(Hip3LiquidatorTransfer { dex: dex.to_owned(), ntl: exact_units(amount, LIQUIDATOR_DECIMALS)?, is_deposit })
    }

    /// The dex whose liquidator it moves money for.
    #[must_use]
    pub fn dex(&self) -> &str {
        &self.dex
    }

    /// The amount as the action carries it, in millionths of the quote token.
    #[must_use]
    pub const fn ntl(&self) -> u64 {
        self.ntl
    }

    /// Whether the money goes in rather than out.
    #[must_use]
    pub const fn is_deposit(&self) -> bool {
        self.is_deposit
    }
}

impl L1Action for Hip3LiquidatorTransfer {
    fn action(&self) -> Map {
        Map::new()
            .put("type", "hip3LiquidatorTransfer")
            .put("dex", self.dex.clone())
            .put("ntl", self.ntl)
            .put("isDeposit", self.is_deposit)
    }
}

#[cfg(test)]
#[path = "tests/liquidator.rs"]
mod tests;
