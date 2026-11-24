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
    /// The fill price (`px`).
    pub px: Dec,
    /// The size filled (`sz`), in the coin's units.
    pub sz: Dec,
    /// The account's side.
    pub side: Side,
    /// Milliseconds.
    pub time: u64,
    /// The account's position in the coin before this fill, signed.
    pub start_position: Dec,
    /// For display: `Open Long`, `Close Short`, `Buy`, `Spot Dust Conversion`, `Settlement`...
    pub dir: String,
    /// The profit or loss this fill realised by closing (`closedPnl`), in the quote token; zero
    /// when it only opened.
    pub closed_pnl: Dec,
    /// The L1 transaction hash; all zeros for a TWAP slice.
    pub hash: String,
    /// The id of the order that filled (`oid`).
    pub oid: u64,
    /// Took liquidity.
    pub crossed: bool,
    /// Negative is a rebate. Includes `builder_fee`.
    pub fee: Dec,
    /// The trade id (`tid`), the same on both sides of the trade.
    pub tid: u64,
    /// The order's client id as written (`0x` and 32 hex digits), kept as a string so a fill reads
    /// whatever its order was tagged with.
    #[serde(default)]
    pub cloid: Option<String>,
    /// The token `fee` is paid in (`feeToken`): `USDC`, or on a spot buy the token bought.
    pub fee_token: String,
    /// What the order's builder took; the venue leaves it out when nothing.
    #[serde(default)]
    pub builder_fee: Option<Dec>,
    /// Only on a fill that was part of a liquidation.
    #[serde(default)]
    pub liquidation: Option<FillLiquidation>,
    /// The TWAP the fill belongs to (`twapId`); `None` for an ordinary order.
    #[serde(default)]
    pub twap_id: Option<u64>,
}

/// On a fill that was part of a liquidation, on either side of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillLiquidation {
    /// The account that was liquidated (`liquidatedUser`); `None` when the venue leaves it out.
    #[serde(default)]
    pub liquidated_user: Option<Address>,
    /// The mark price at the liquidation (`markPx`).
    pub mark_px: Dec,
    /// `market` or `backstop`.
    pub method: String,
}

/// A fill of one of a TWAP's slices.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFill {
    /// The slice's fill, in the same shape as any other fill.
    pub fill: Fill,
    /// The TWAP the slice belongs to (`twapId`).
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
    /// The base rates and tiers every account is billed from (`feeSchedule`).
    #[serde(default)]
    pub fee_schedule: FeeSchedule,
    /// Taking on perps.
    #[serde(default)]
    pub user_cross_rate: Dec,
    /// Making on perps; below zero, a rebate.
    #[serde(default)]
    pub user_add_rate: Dec,
    /// Taking on spot and outcomes (`userSpotCrossRate`).
    #[serde(default)]
    pub user_spot_cross_rate: Dec,
    /// Making on spot and outcomes (`userSpotAddRate`); below zero, a rebate.
    #[serde(default)]
    pub user_spot_add_rate: Dec,
    /// The fraction taken off fees for a referral (`activeReferralDiscount`); zero when none.
    #[serde(default)]
    pub active_referral_discount: Dec,
    /// The staking tier that applies (`activeStakingDiscount`); `None` when the account stakes too
    /// little for one.
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
    /// The account's taking volume that day (`userCross`), in USDC.
    #[serde(default)]
    pub user_cross: Dec,
    /// The account's making volume that day (`userAdd`), in USDC.
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
    /// The base rate for taking on perps.
    #[serde(default)]
    pub cross: Dec,
    /// The base rate for making on perps.
    #[serde(default)]
    pub add: Dec,
    /// The base rate for taking on spot (`spotCross`).
    #[serde(default)]
    pub spot_cross: Dec,
    /// The base rate for making on spot (`spotAdd`).
    #[serde(default)]
    pub spot_add: Dec,
    /// The tiers volume and making earn.
    #[serde(default)]
    pub tiers: FeeTiers,
    /// The fraction a referral takes off fees (`referralDiscount`).
    #[serde(default)]
    pub referral_discount: Dec,
    /// The staking tiers, each a discount for staking a share of HYPE's supply
    /// (`stakingDiscountTiers`).
    #[serde(default)]
    pub staking_discount_tiers: Vec<StakingDiscount>,
}

/// The fee tiers: by volume, and by share of the venue's maker volume.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct FeeTiers {
    /// The tiers by fourteen days' volume (`vip`), lowest first.
    pub vip: Vec<VolumeTier>,
    /// The maker rebate tiers by share of the venue's maker volume (`mm`), lowest first.
    pub mm: Vec<MakerTier>,
}

/// A rate by fourteen days' volume.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeTier {
    /// The fourteen days' volume, in USDC, from which this tier applies (`ntlCutoff`).
    pub ntl_cutoff: Dec,
    /// The rate for taking on perps.
    pub cross: Dec,
    /// The rate for making on perps.
    pub add: Dec,
    /// The rate for taking on spot (`spotCross`).
    pub spot_cross: Dec,
    /// The rate for making on spot (`spotAdd`).
    pub spot_add: Dec,
}

/// A maker rebate by share of the venue's maker volume.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakerTier {
    /// The fraction of the venue's maker volume from which this tier applies
    /// (`makerFractionCutoff`).
    pub maker_fraction_cutoff: Dec,
    /// The rate for making on perps at this tier; below zero, a rebate.
    pub add: Dec,
}

/// A fee discount for staking HYPE: how much stake earns it and how much it takes off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingDiscount {
    /// The stake needed, in basis points of HYPE's maximum supply (`bpsOfMaxSupply`).
    #[serde(default)]
    pub bps_of_max_supply: Dec,
    /// The fraction taken off fees, `0.05` for 5%.
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

/// Reads `userFills`; see [`user_fills_request`].
pub async fn user_fills(transport: &impl Transport, account: Address, aggregate: bool) -> Result<Vec<Fill>, ApiError> {
    info(transport, user_fills_request(account, aggregate)).await
}

/// Reads `userFillsByTime`; see [`user_fills_by_time_request`].
pub async fn user_fills_by_time(
    transport: &impl Transport,
    account: Address,
    start: u64,
    end: Option<u64>,
    aggregate: bool,
) -> Result<Vec<Fill>, ApiError> {
    info(transport, user_fills_by_time_request(account, start, end, aggregate)).await
}

/// Reads `userTwapSliceFills`; see [`user_twap_slice_fills_request`].
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
