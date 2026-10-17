//! What an outcome fill is billed, from the account's `userFees` ([`UserFees`]): the spot rate
//! (rested or crossed) times `scale + max(scale, 1)` times one less any referral discount, against
//! notional, and on the selling side only. No endpoint reports what a deployer earned, so
//! [`deployer_share`] is an estimate.

use crate::account::UserFees;
use crate::account::holdings::outcomes::Fill;
use crate::trade::Side;
use crate::utils::{Dec, Round};

/// `scale + max(scale, 1)`: 2 at the scale every live outcome carried.
#[must_use]
pub fn multiplier(scale: Dec) -> Option<Dec> {
    scale.checked_add(scale.max(Dec::ONE))
}

/// The deployer's part of `fee` on an outcome of fee scale `scale`, `fee *
/// scale / (scale + max(scale, 1))`: multiplied out before the one division,
/// then cut to eight places toward zero, so it rounds once. An estimate: the
/// venue reports none.
#[must_use]
pub fn deployer_share(fee: Dec, scale: Dec) -> Option<Dec> {
    fee.checked_mul(scale)?.checked_div(multiplier(scale)?, 8, Round::Down)
}

/// The rate a fill on an outcome of fee scale `scale` is billed at, had it been billed: the
/// rested or the crossed spot rate.
#[must_use]
pub fn rate(fees: &UserFees, crossed: bool, scale: Dec) -> Option<Dec> {
    let base = if crossed { fees.user_spot_cross_rate } else { fees.user_spot_add_rate };
    let kept = Dec::ONE.checked_sub(fees.active_referral_discount)?;
    base.checked_mul(multiplier(scale)?)?.checked_mul(kept)
}

/// What a fill of `notional` is billed: nothing on a buy, the rate times the notional on a sell.
#[must_use]
pub fn billed(fees: &UserFees, side: Side, crossed: bool, scale: Dec, notional: Dec) -> Option<Dec> {
    match side {
        Side::Buy => Some(Dec::ZERO),
        Side::Sell => rate(fees, crossed, scale)?.checked_mul(notional),
    }
}

/// [`billed`] for a fill as read, the builder's fee left out.
#[must_use]
pub fn billed_for(fees: &UserFees, fill: &Fill, scale: Dec) -> Option<Dec> {
    billed(fees, fill.side, fill.crossed, scale, fill.notional())
}

#[cfg(test)]
#[path = "tests/fee.rs"]
mod tests;
