//! An account's fills and what it pays for them, on every market kind: `userFills`,
//! `userFillsByTime`, `userTwapSliceFills` and `userFees`. A fill is the same shape whether a read
//! answers it or the socket sends it; perp, spot and outcome fills come in the same answer.

use serde::Deserialize;

use crate::client::{ApiError, Transport, info, info_request, user_request};
use crate::markets::Side;
use crate::utils::{Address, Dec, Json};

/// One fill. `fee` is everything the fill cost, a builder's share included, in `fee_token`:
/// USDC, or on a spot buy the token bought.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    /// `BTC`, `xyz:XYZ100`, `@107`, `#14730`.
    pub coin: String,
    pub px: Dec,
    pub sz: Dec,
    /// The account's side.
    pub side: Side,
    /// Milliseconds.
    pub time: u64,
    /// The account's position in the coin before this fill, signed.
    pub start_position: Dec,
    /// For display: `Open Long`, `Close Short`, `Buy`, `Spot Dust Conversion`, `Settlement`...
    pub dir: String,
    pub closed_pnl: Dec,
    /// The L1 transaction hash; all zeros for a TWAP slice.
    pub hash: String,
    pub oid: u64,
    /// Took liquidity.
    pub crossed: bool,
    /// Negative is a rebate. Includes `builder_fee`.
    pub fee: Dec,
    pub tid: u64,
    /// The order's client id as written (`0x` and 32 hex digits), kept as a string so a fill reads
    /// whatever its order was tagged with.
    #[serde(default)]
    pub cloid: Option<String>,
    pub fee_token: String,
    /// What the order's builder took; the venue leaves it out when nothing.
    #[serde(default)]
    pub builder_fee: Option<Dec>,
    /// Only on a fill that was part of a liquidation.
    #[serde(default)]
    pub liquidation: Option<FillLiquidation>,
    #[serde(default)]
    pub twap_id: Option<u64>,
}

/// On a fill that was part of a liquidation, on either side of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillLiquidation {
    #[serde(default)]
    pub liquidated_user: Option<Address>,
    pub mark_px: Dec,
    /// `market` or `backstop`.
    pub method: String,
}

/// A fill of one of a TWAP's slices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFill {
    pub fill: Fill,
    pub twap_id: u64,
}

/// `userFees`: the account's own rates, as fractions of a fill's value (`0.00045` is 4.5 basis
/// points), its recent volume and the schedule. Spot and HIP-4 outcomes bill off the spot rates.
/// A figure left out reads as zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFees {
    /// The days the tier is read over, today last.
    #[serde(default)]
    pub daily_user_vlm: Vec<DailyVolume>,
    #[serde(default)]
    pub fee_schedule: FeeSchedule,
    /// Taking on perps.
    #[serde(default)]
    pub user_cross_rate: Dec,
    /// Making on perps; below zero, a rebate.
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

impl UserFees {
    /// Every day's taking and making, summed. `None` only past what a `Dec` counts.
    #[must_use]
    pub fn volume(&self) -> Option<Dec> {
        Dec::checked_sum(self.daily_user_vlm.iter().flat_map(|day| [day.user_cross, day.user_add]))
    }
}

/// One day's volume: the account's taking and making, and the venue's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyVolume {
    /// `2026-09-24`.
    pub date: String,
    #[serde(default)]
    pub user_cross: Dec,
    #[serde(default)]
    pub user_add: Dec,
    /// The whole venue's.
    #[serde(default)]
    pub exchange: Dec,
}

/// The base tier every account starts from, and the tiers volume and making earn.
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
    pub tiers: FeeTiers,
    #[serde(default)]
    pub referral_discount: Dec,
    #[serde(default)]
    pub staking_discount_tiers: Vec<StakingDiscount>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingDiscount {
    #[serde(default)]
    pub bps_of_max_supply: Dec,
    #[serde(default)]
    pub discount: Dec,
}

fn user(account: Address) -> (&'static str, Json) {
    ("user", Json::String(account.to_hex()))
}

/// `aggregateByTime` when asked: a crossing order's partial fills combined.
fn aggregated(aggregate: bool) -> Option<(&'static str, Json)> {
    aggregate.then_some(("aggregateByTime", Json::Bool(true)))
}

/// `{"type":"userFills","user":..,"aggregateByTime"?}`: at most the 2000
/// most recent, newest first.
#[must_use]
pub fn user_fills_request(account: Address, aggregate: bool) -> String {
    let fields: Vec<_> = [user(account)].into_iter().chain(aggregated(aggregate)).collect();
    info_request("userFills", &fields)
}

/// `{"type":"userFillsByTime","user":..,"startTime":..,"endTime"?,
/// "aggregateByTime"?}`, both times unix milliseconds and inclusive: at most
/// 2000 a read, oldest first, from the 10000 most recent.
#[must_use]
pub fn user_fills_by_time_request(account: Address, start: u64, end: Option<u64>, aggregate: bool) -> String {
    let number = |value: u64| Json::Number(value.to_string());
    let window = [Some(("startTime", number(start))), end.map(|end| ("endTime", number(end)))];
    let fields: Vec<_> =
        [user(account)].into_iter().chain(window.into_iter().flatten()).chain(aggregated(aggregate)).collect();
    info_request("userFillsByTime", &fields)
}

/// `{"type":"userTwapSliceFills","user":..}`: at most the 2000 most recent.
#[must_use]
pub fn user_twap_slice_fills_request(account: Address) -> String {
    user_request("userTwapSliceFills", account)
}

/// `{"type":"userFees","user":..}`.
#[must_use]
pub fn user_fees_request(account: Address) -> String {
    user_request("userFees", account)
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

/// Weighs 20.
pub async fn user_fees(transport: &impl Transport, account: Address) -> Result<UserFees, ApiError> {
    info(transport, user_fees_request(account)).await
}

#[cfg(test)]
#[path = "tests/fills.rs"]
mod tests;
