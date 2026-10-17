//! What an outcome fill is billed: the spot rate (rested or crossed) times `scale + max(scale, 1)`
//! times one less any referral discount, against notional, and on the selling side only. No
//! endpoint reports what a deployer earned, so [`deployer_share`] is an estimate.

use crate::account::outcomes::Fill;
use crate::client::{ApiError, Transport, info, user_request};
use crate::trade::Side;
use crate::utils::{Address, Dec, Round};
use serde::Deserialize;

/// `{"type":"userFees","user":..}`.
#[must_use]
pub fn user_fees_request(user: Address) -> String {
    user_request("userFees", user)
}

pub async fn user_fees(transport: &impl Transport, user: Address) -> Result<UserFees, ApiError> {
    info(transport, user_fees_request(user)).await
}

/// The part of `userFees` an outcome fill is billed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct UserFees {
    #[serde(rename = "userSpotCrossRate")]
    pub spot_cross: Dec,
    #[serde(rename = "userSpotAddRate")]
    pub spot_add: Dec,
    #[serde(default, rename = "activeReferralDiscount")]
    pub referral_discount: Dec,
}

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

impl UserFees {
    /// The rate a fill on an outcome of fee scale `scale` is billed at, had
    /// it been billed: the rested or the crossed spot rate.
    #[must_use]
    pub fn rate(&self, crossed: bool, scale: Dec) -> Option<Dec> {
        let base = if crossed { self.spot_cross } else { self.spot_add };
        let kept = Dec::ONE.checked_sub(self.referral_discount)?;
        base.checked_mul(multiplier(scale)?)?.checked_mul(kept)
    }

    /// What a fill of `notional` is billed: nothing on a buy, the rate times
    /// the notional on a sell.
    #[must_use]
    pub fn fee(&self, side: Side, crossed: bool, scale: Dec, notional: Dec) -> Option<Dec> {
        match side {
            Side::Buy => Some(Dec::ZERO),
            Side::Sell => self.rate(crossed, scale)?.checked_mul(notional),
        }
    }

    /// [`UserFees::fee`] for a fill as read, the builder's fee left out.
    #[must_use]
    pub fn fee_of(&self, fill: &Fill, scale: Dec) -> Option<Dec> {
        self.fee(fill.side, fill.crossed, scale, fill.notional())
    }
}

#[cfg(test)]
#[path = "tests/fee.rs"]
mod tests;
