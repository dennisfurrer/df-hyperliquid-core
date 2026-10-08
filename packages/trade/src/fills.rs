//! An account's fills and what it pays for them, on every market kind:
//! `userFills`, `userFillsByTime`, `userTwapSliceFills` and `userFees`.

use hl_core::{Address, ApiError, Dec, Json, Transport, info};
use serde::Deserialize;

use crate::{
    order::{Cloid, Side},
    request::{body, number, user},
};

/// One fill. `fee` is everything the fill cost, a builder's share included,
/// in `fee_token`: USDC, or on a spot buy the token bought.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    pub coin: String,
    pub px: Dec,
    pub sz: Dec,
    pub side: Side,
    pub time: u64,
    /// The position before the fill, signed.
    pub start_position: Dec,
    /// `Open Long`, `Close Short`, `Buy`, `Sell`, `Spot Dust Conversion`...
    pub dir: String,
    pub closed_pnl: Dec,
    /// All zeros on a TWAP slice.
    pub hash: String,
    pub oid: u64,
    /// Took liquidity.
    pub crossed: bool,
    pub fee: Dec,
    pub fee_token: String,
    /// What the order's builder took; the venue leaves it out when nothing.
    #[serde(default)]
    pub builder_fee: Option<Dec>,
    pub tid: u64,
    #[serde(default)]
    pub cloid: Option<Cloid>,
    #[serde(default)]
    pub twap_id: Option<u64>,
}

/// A fill of one of a TWAP's slices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFill {
    pub fill: Fill,
    pub twap_id: u64,
}

/// `userFees`: the account's own rates, as fractions of a fill's value
/// (`0.00045` is 4.5 basis points), its recent volume and the schedule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFees {
    #[serde(default)]
    pub daily_user_vlm: Vec<DailyVolume>,
    pub fee_schedule: FeeSchedule,
    /// Taking on perps.
    pub user_cross_rate: Dec,
    /// Making on perps; below zero, a rebate.
    pub user_add_rate: Dec,
    pub user_spot_cross_rate: Dec,
    pub user_spot_add_rate: Dec,
    pub active_referral_discount: Dec,
    #[serde(default)]
    pub active_staking_discount: Option<StakingDiscount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyVolume {
    /// `2026-09-24`.
    pub date: String,
    pub user_cross: Dec,
    pub user_add: Dec,
    /// The whole venue's.
    pub exchange: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeSchedule {
    pub cross: Dec,
    pub add: Dec,
    pub spot_cross: Dec,
    pub spot_add: Dec,
    pub tiers: FeeTiers,
    pub referral_discount: Dec,
    #[serde(default)]
    pub staking_discount_tiers: Vec<StakingDiscount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeeTiers {
    pub vip: Vec<VolumeTier>,
    pub mm: Vec<MakerTier>,
}

/// A rate by fourteen days' volume.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeTier {
    pub ntl_cutoff: Dec,
    pub cross: Dec,
    pub add: Dec,
    pub spot_cross: Dec,
    pub spot_add: Dec,
}

/// A maker rebate by share of the venue's maker volume.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakerTier {
    pub maker_fraction_cutoff: Dec,
    pub add: Dec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingDiscount {
    pub bps_of_max_supply: Dec,
    pub discount: Dec,
}

/// `aggregateByTime` when asked: a crossing order's partial fills combined.
fn aggregated(aggregate: bool) -> Option<(&'static str, Json)> {
    aggregate.then_some(("aggregateByTime", Json::Bool(true)))
}

/// `{"type":"userFills","user":..,"aggregateByTime"?}`: at most the 2000
/// most recent, newest first.
#[must_use]
pub fn user_fills_request(account: Address, aggregate: bool) -> String {
    body("userFills", [user(account)].into_iter().chain(aggregated(aggregate)))
}

/// `{"type":"userFillsByTime","user":..,"startTime":..,"endTime"?,
/// "aggregateByTime"?}`, both times unix milliseconds and inclusive: at most
/// 2000 a read, oldest first, from the 10000 most recent.
#[must_use]
pub fn user_fills_by_time_request(account: Address, start: u64, end: Option<u64>, aggregate: bool) -> String {
    let window = [Some(("startTime", number(start))), end.map(|end| ("endTime", number(end)))];
    body(
        "userFillsByTime",
        [user(account)].into_iter().chain(window.into_iter().flatten()).chain(aggregated(aggregate)),
    )
}

/// `{"type":"userTwapSliceFills","user":..}`: at most the 2000 most recent.
#[must_use]
pub fn user_twap_slice_fills_request(account: Address) -> String {
    body("userTwapSliceFills", [user(account)])
}

/// `{"type":"userFees","user":..}`.
#[must_use]
pub fn user_fees_request(account: Address) -> String {
    body("userFees", [user(account)])
}

pub async fn user_fills(transport: &impl Transport, account: Address, aggregate: bool) -> Result<Vec<Fill>, ApiError> {
    info(transport, user_fills_request(account, aggregate)).await
}

pub async fn user_fills_by_time(
    transport: &impl Transport,
    account: Address,
    start: u64,
    end: Option<u64>,
    aggregate: bool,
) -> Result<Vec<Fill>, ApiError> {
    info(transport, user_fills_by_time_request(account, start, end, aggregate)).await
}

pub async fn user_twap_slice_fills(
    transport: &impl Transport,
    account: Address,
) -> Result<Vec<TwapSliceFill>, ApiError> {
    info(transport, user_twap_slice_fills_request(account)).await
}

pub async fn user_fees(transport: &impl Transport, account: Address) -> Result<UserFees, ApiError> {
    info(transport, user_fees_request(account)).await
}

#[cfg(test)]
#[path = "tests/fills.rs"]
mod tests;
