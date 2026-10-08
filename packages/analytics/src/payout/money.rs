//! USDC as a payout counts it: a plain decimal, never below zero, kept to the six places USDC
//! has, whatever was typed past them cut rather than rounded up.

use hl_core::{Dec, Round};

use crate::math::add;

/// USDC's own precision.
pub const USDC_DECIMALS: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadUsdc {
    #[error("`{0}` is not a decimal amount of USDC")]
    NotANumber(String),
    #[error("`{0}` is negative, and a payout is never negative")]
    Negative(String),
    #[error("`{0}` is more USDC than this build can count")]
    OutOfRange(String),
}

/// Reads an amount of USDC, cutting past six places rather than rounding. Its millionths must
/// fit a `u64`, the integer an amount of USDC is counted in on chain.
pub fn parse(text: &str) -> Result<Dec, BadUsdc> {
    let said = text.trim();
    if said.starts_with('-') {
        return Err(BadUsdc::Negative(said.to_owned()));
    }
    let body = said.strip_prefix('+').unwrap_or(said);
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let shaped = !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit());
    if !shaped {
        return Err(BadUsdc::NotANumber(said.to_owned()));
    }
    let kept = fraction.get(..fraction.len().min(USDC_DECIMALS as usize)).unwrap_or("");
    let amount = Dec::parse(&format!("{whole}.{kept}0")).map_err(|_| BadUsdc::OutOfRange(said.to_owned()))?;
    match micros(amount) {
        Some(_) => Ok(amount),
        None => Err(BadUsdc::OutOfRange(said.to_owned())),
    }
}

/// The amount as a payout writes it: six places at most, no trailing zeros.
#[must_use]
pub fn wire(amount: Dec) -> String {
    amount.round(USDC_DECIMALS, Round::Down).to_wire()
}

/// The amount in millionths of USDC, cut down; `None` below zero or past a `u64`.
#[must_use]
pub fn micros(amount: Dec) -> Option<u64> {
    u64::try_from(amount.units_at(USDC_DECIMALS, Round::Down)?).ok()
}

/// Whether USDC can pay it as it is: six places at most.
#[must_use]
pub fn is_payable(amount: Dec) -> bool {
    amount.places() <= USDC_DECIMALS
}

/// Every amount added, exactly.
#[must_use]
pub fn total(amounts: impl IntoIterator<Item = Dec>) -> Dec {
    amounts.into_iter().fold(Dec::ZERO, add)
}

#[cfg(test)]
#[path = "../tests/money.rs"]
mod tests;
