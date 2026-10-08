//! What an outcome fill is billed (D11).
//!
//! The HIP-4 page says outcome markets are free; that is stale. They bill off
//! the spot schedule, `userSpotAddRate` for a fill that rested and
//! `userSpotCrossRate` for one that crossed (not the perp rates), times
//! `scale + max(scale, 1)` for the deployer's cut, times one less any
//! referral discount, against notional:
//!
//! ```text
//! rate = spot_rate * (scale + max(scale, 1)) * (1 - referral_discount)
//! ```
//!
//! and on the closing side only. Across 1,982 fills on one account every buy
//! was billed 0 and every sell was not; 7,256 on four accounts agreed. A model
//! charging both sides is wrong by a factor of two. No endpoint reports what
//! a deployer earned, so [`deployer_share`] is an estimate.

use hl_core::{Address, ApiError, Dec, Round, Transport, info, user_request};
use hl_trade::Side;
use serde::Deserialize;

use crate::fill::Fill;

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

/// The deployer's part of an outcome's fee, `scale / (scale + max(scale,
/// 1))`, to eight places toward zero. An estimate: the venue reports none.
#[must_use]
pub fn deployer_share(scale: Dec) -> Option<Dec> {
    scale.checked_div(multiplier(scale)?, 8, Round::Down)
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
