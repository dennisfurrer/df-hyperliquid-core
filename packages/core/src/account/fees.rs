//! `userFees`: what an account pays, and the volume its tier is read over. Rates are fractions of
//! notional (`0.00045` is 4.5 basis points); spot and HIP-4 outcomes bill off the spot rates.

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `{"type":"userFees","user":..}`.
#[must_use]
pub fn fees_request(user: Address) -> String {
    user_request("userFees", user)
}

/// Weighs 20.
pub async fn fees(transport: &impl Transport, user: Address) -> Result<Fees, ApiError> {
    info(transport, fees_request(user)).await
}

/// `userFees`. The schedule's tiers, the trial and the staking link's kind
/// are left unread.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fees {
    /// The days the tier is read over, today last.
    #[serde(default)]
    pub daily_user_vlm: Vec<DayVolume>,
    #[serde(default)]
    pub fee_schedule: Option<FeeSchedule>,
    /// Taking, on perps.
    #[serde(default)]
    pub user_cross_rate: Dec,
    /// Making, on perps; below zero, a rebate.
    #[serde(default)]
    pub user_add_rate: Dec,
    #[serde(default)]
    pub user_spot_cross_rate: Dec,
    #[serde(default)]
    pub user_spot_add_rate: Dec,
    #[serde(default)]
    pub active_referral_discount: Dec,
    #[serde(default)]
    pub active_staking_discount: Option<StakingDiscount>,
}

/// One day's volume: the account's taking and making, and the venue's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayVolume {
    pub date: String,
    #[serde(default)]
    pub user_cross: Dec,
    #[serde(default)]
    pub user_add: Dec,
    #[serde(default)]
    pub exchange: Dec,
}

/// The base tier every account starts from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeSchedule {
    #[serde(default)]
    pub cross: Dec,
    #[serde(default)]
    pub add: Dec,
    #[serde(default)]
    pub spot_cross: Dec,
    #[serde(default)]
    pub spot_add: Dec,
    #[serde(default)]
    pub referral_discount: Option<Dec>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingDiscount {
    #[serde(default)]
    pub bps_of_max_supply: Dec,
    #[serde(default)]
    pub discount: Dec,
}

impl Fees {
    /// Every day's taking and making, summed. `None` only past what a `Dec`
    /// counts.
    #[must_use]
    pub fn volume(&self) -> Option<Dec> {
        Dec::checked_sum(self.daily_user_vlm.iter().flat_map(|day| [day.user_cross, day.user_add]))
    }
}

/// A rate in basis points: `0.00045` is 4.5.
#[must_use]
pub fn bps(rate: Dec) -> Option<Dec> {
    rate.checked_mul(Dec::from_int(10_000))
}

#[cfg(test)]
#[path = "tests/fees.rs"]
mod tests;
