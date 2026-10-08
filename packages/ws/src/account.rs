//! What the websocket says about one account: fills, orders, events, funding, the ledger, each
//! dex's clearinghouse, spot balances, TWAPs, and what it may trade. Amounts are exact [`Dec`];
//! the venue writes them as strings. Frames that open with history say so in `is_snapshot`.

use std::fmt;

use hl_core::{Address, Dec, Json};
use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::market::Side;

/// One fill of the account's, as `userFills`, `userEvents` and the TWAP feeds send it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    pub coin: String,
    pub px: Dec,
    pub sz: Dec,
    /// The account's side.
    pub side: Side,
    /// Milliseconds.
    pub time: u64,
    /// The account's position in the coin before this fill, signed.
    pub start_position: Dec,
    /// For display: `Open Long`, `Close Short`, `Buy`, `Settlement` and the like.
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
    #[serde(default)]
    pub cloid: Option<String>,
    pub fee_token: String,
    #[serde(default)]
    pub builder_fee: Option<Dec>,
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

/// `userFills`: the last fills first (`is_snapshot`), then each new one as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFills {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub fills: Vec<Fill>,
}

/// An order as `orderUpdates` sends it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub coin: String,
    pub side: Side,
    pub limit_px: Dec,
    /// What is left.
    pub sz: Dec,
    pub oid: u64,
    /// When it was placed, milliseconds.
    pub timestamp: u64,
    pub orig_sz: Dec,
    #[serde(default)]
    pub cloid: Option<String>,
    #[serde(default)]
    pub reduce_only: bool,
}

/// One order's new status on `orderUpdates`: `open`, `filled`, `canceled`, `triggered`,
/// `rejected`, `marginCanceled`, `badAloPxRejected` and the rest the info docs list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdate {
    pub order: Order,
    pub status: String,
    /// Milliseconds.
    pub status_timestamp: u64,
}

/// A resting order with its trigger and type, as `openOrders` and `userHistoricalOrders` send
/// it (the info endpoint's `frontendOpenOrders` shape).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    pub coin: String,
    pub side: Side,
    pub limit_px: Dec,
    /// What is left; `0` on a position TP/SL, which closes the whole position.
    pub sz: Dec,
    pub oid: u64,
    /// Milliseconds.
    pub timestamp: u64,
    /// `N/A` unless a trigger order.
    #[serde(default)]
    pub trigger_condition: String,
    #[serde(default)]
    pub is_trigger: bool,
    #[serde(default)]
    pub trigger_px: Dec,
    /// A parent's take-profit and stop-loss.
    #[serde(default)]
    pub children: Vec<OpenOrder>,
    #[serde(default)]
    pub is_position_tpsl: bool,
    #[serde(default)]
    pub reduce_only: bool,
    /// `Limit`, `Stop Market`, `Take Profit Limit` and the like.
    #[serde(default)]
    pub order_type: String,
    pub orig_sz: Dec,
    /// `Gtc`, `Alo`, `Ioc`, `FrontendMarket`; none on a trigger order.
    #[serde(default)]
    pub tif: Option<String>,
    #[serde(default)]
    pub cloid: Option<String>,
}

/// `userEvents`, on the channel `user`: one kind of event a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    Fills(Vec<Fill>),
    Funding(Funding),
    Liquidation(Liquidation),
    /// Orders the venue canceled, not the account.
    NonUserCancel(Vec<Cancel>),
    TwapHistory(Vec<TwapEvent>),
    TwapSliceFills(Vec<TwapSliceFill>),
    /// A kind this build does not read: its key, and its value as written.
    Other {
        key: String,
        data: String,
    },
}

impl<'de> Deserialize<'de> for UserEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Event;

        impl<'de> de::Visitor<'de> for Event {
            type Value = UserEvent;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object with one event in it")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<UserEvent, A::Error> {
                let key: String = map.next_key()?.ok_or_else(|| de::Error::custom("an event with nothing in it"))?;
                let event = match key.as_str() {
                    "fills" => UserEvent::Fills(map.next_value()?),
                    "funding" => UserEvent::Funding(map.next_value()?),
                    "liquidation" => UserEvent::Liquidation(map.next_value()?),
                    "nonUserCancel" => UserEvent::NonUserCancel(map.next_value()?),
                    "twapHistory" => UserEvent::TwapHistory(map.next_value()?),
                    "twapSliceFills" => UserEvent::TwapSliceFills(map.next_value()?),
                    _ => {
                        let data: Box<RawValue> = map.next_value()?;
                        UserEvent::Other { key, data: data.get().to_owned() }
                    }
                };
                if map.next_key::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("more than one event in a frame"));
                }
                Ok(event)
            }
        }

        deserializer.deserialize_map(Event)
    }
}

/// One funding payment: on `userFundings`, and as a `userEvents` event on the hour.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Funding {
    /// Milliseconds.
    pub time: u64,
    pub coin: String,
    /// Paid out (negative) or received.
    pub usdc: Dec,
    /// The position it was paid on, signed.
    pub szi: Dec,
    pub funding_rate: Dec,
    #[serde(default)]
    pub n_samples: Option<u64>,
}

/// A liquidation the account took part in. The venue writes these keys in snake case.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Liquidation {
    pub lid: u64,
    pub liquidator: Address,
    pub liquidated_user: Address,
    pub liquidated_ntl_pos: Dec,
    pub liquidated_account_value: Dec,
}

/// An order the venue canceled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cancel {
    pub coin: String,
    pub oid: u64,
}

/// `userFundings`: past payments first (`is_snapshot`), then one an hour per position.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFundings {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub fundings: Vec<Funding>,
}

/// `userNonFundingLedgerUpdates`: deposits, withdrawals, transfers, sends, vault moves and
/// liquidations; past ones first (`is_snapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "nonFundingLedgerUpdates")]
    pub updates: Vec<LedgerUpdate>,
}

/// One ledger row. The `delta` is kept as written, every key and digit: its types are many and
/// grow, and `hl-transfer` reads the same rows into its own `Delta`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LedgerUpdate {
    /// Milliseconds.
    pub time: u64,
    pub hash: String,
    #[serde(deserialize_with = "json")]
    pub delta: Json,
}

impl LedgerUpdate {
    /// The row's `type`: `deposit`, `send`, `withdraw`, `accountClassTransfer` and the rest.
    #[must_use]
    pub fn kind(&self) -> Option<&str> {
        self.delta.get("type").and_then(Json::as_str)
    }

    /// One of the delta's amounts (`usdc`, `amount`, `fee`), exact; `None` when it is absent or
    /// not a decimal.
    #[must_use]
    pub fn dec(&self, key: &str) -> Option<Dec> {
        match self.delta.get(key)? {
            Json::String(text) | Json::Number(text) => Dec::parse(text).ok(),
            _ => None,
        }
    }
}

/// A value read into hl-core's order-keeping tree, numbers kept as their digits.
fn json<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
    let raw = Box::<RawValue>::deserialize(deserializer)?;
    Json::parse(raw.get()).map_err(de::Error::custom)
}

/// One dex's clearinghouse: the account's margin and positions there.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clearinghouse {
    pub margin_summary: MarginSummary,
    pub cross_margin_summary: MarginSummary,
    pub cross_maintenance_margin_used: Dec,
    pub withdrawable: Dec,
    pub asset_positions: Vec<AssetPosition>,
    /// Milliseconds.
    pub time: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    pub account_value: Dec,
    pub total_ntl_pos: Dec,
    pub total_raw_usd: Dec,
    pub total_margin_used: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetPosition {
    /// `oneWay`.
    #[serde(rename = "type")]
    pub kind: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub coin: String,
    /// Signed size.
    pub szi: Dec,
    pub leverage: Leverage,
    #[serde(default)]
    pub entry_px: Option<Dec>,
    pub position_value: Dec,
    pub unrealized_pnl: Dec,
    pub return_on_equity: Dec,
    #[serde(default)]
    pub liquidation_px: Option<Dec>,
    pub margin_used: Dec,
    pub max_leverage: u32,
    #[serde(default)]
    pub cum_funding: CumFunding,
}

/// A position's leverage, or what the account has set on a coin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leverage {
    #[serde(rename = "type")]
    pub margin: Margin,
    pub value: u32,
    /// An isolated position's own USD.
    #[serde(default)]
    pub raw_usd: Option<Dec>,
}

/// Whether a position shares the account's balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Margin {
    Cross,
    Isolated,
}

/// Funding paid (positive) or received on a position.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CumFunding {
    pub all_time: Dec,
    pub since_open: Dec,
    pub since_change: Dec,
}

/// `clearinghouseState`: one dex's clearinghouse (`dex` empty for the first).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexState {
    pub dex: String,
    pub user: Address,
    #[serde(rename = "clearinghouseState")]
    pub state: Clearinghouse,
}

/// `allDexsClearinghouseState`: every dex's clearinghouse, the first named `""`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllDexsState {
    pub user: Address,
    #[serde(rename = "clearinghouseStates")]
    pub states: Vec<(String, Clearinghouse)>,
}

/// `openOrders`: every resting order on one dex.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OpenOrders {
    pub dex: String,
    pub user: Address,
    pub orders: Vec<OpenOrder>,
}

/// `spotState`: spot balances, outcome sides among them (`+14720`, with no token).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawSpotState")]
pub struct SpotState {
    pub user: Address,
    pub balances: Vec<Balance>,
    /// Pooled accounts: what each token (by index) has free before maintenance.
    pub available_after_maintenance: Vec<(u32, Dec)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSpotState {
    user: Address,
    spot_state: RawBalances,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBalances {
    balances: Vec<Balance>,
    #[serde(default)]
    token_to_available_after_maintenance: Vec<(u32, Dec)>,
}

impl From<RawSpotState> for SpotState {
    fn from(raw: RawSpotState) -> Self {
        SpotState {
            user: raw.user,
            balances: raw.spot_state.balances,
            available_after_maintenance: raw.spot_state.token_to_available_after_maintenance,
        }
    }
}

/// One token's balance. `total` already includes `hold`; under portfolio margin a hold may be
/// negative, room to borrow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub coin: String,
    /// The token's index; outcome sides have none.
    #[serde(default)]
    pub token: Option<u32>,
    pub total: Dec,
    pub hold: Dec,
    #[serde(default)]
    pub entry_ntl: Option<Dec>,
    #[serde(default)]
    pub spot_hold: Option<Dec>,
    #[serde(default)]
    pub ltv: Option<Dec>,
    #[serde(default)]
    pub supplied: Option<Dec>,
    #[serde(default)]
    pub borrowed: Option<Dec>,
}

/// `twapStates`: the account's running TWAPs on one dex, by id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwapStates {
    pub dex: String,
    pub user: Address,
    pub states: Vec<(u64, TwapState)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapState {
    pub coin: String,
    pub user: Address,
    pub side: Side,
    pub sz: Dec,
    pub executed_sz: Dec,
    pub executed_ntl: Dec,
    pub minutes: u32,
    pub reduce_only: bool,
    pub randomize: bool,
    /// When it started, milliseconds.
    pub timestamp: u64,
}

/// One slice of a TWAP that filled.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFill {
    pub fill: Fill,
    pub twap_id: u64,
}

/// `userTwapSliceFills`: past slices first (`is_snapshot`), then each as it fills.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapSliceFills {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "twapSliceFills")]
    pub fills: Vec<TwapSliceFill>,
}

/// A TWAP starting, finishing, being stopped or failing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapEvent {
    /// Seconds, not milliseconds: the venue writes this one so.
    pub time: u64,
    pub state: TwapState,
    pub status: TwapStatus,
    #[serde(default)]
    pub twap_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwapStatus {
    /// `activated`, `finished`, `terminated` or `error`.
    pub status: String,
    /// Why, on an error.
    #[serde(default)]
    pub description: Option<String>,
}

/// `userTwapHistory`: past TWAP events first (`is_snapshot`), then each as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwapHistory {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub history: Vec<TwapEvent>,
}

/// `webData3`: the account as the venue's own app reads it. The docs warn that fields they do
/// not list may go; only the listed ones and `abstraction` are read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebData3 {
    pub user_state: UserState,
    pub perp_dex_states: Vec<PerpDexState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    pub user: Address,
    #[serde(default)]
    pub agent_address: Option<Address>,
    /// Milliseconds.
    #[serde(default)]
    pub agent_valid_until: Option<u64>,
    /// Milliseconds.
    pub server_time: u64,
    pub cum_ledger: Dec,
    pub is_vault: bool,
    /// The account mode: `disabled`, `default`, `unifiedAccount`, `portfolioMargin`.
    #[serde(default)]
    pub abstraction: Option<String>,
    #[serde(default)]
    pub dex_abstraction_enabled: bool,
    #[serde(default)]
    pub opt_out_of_spot_dusting: bool,
}

/// One perp dex, in the order the dexes are listed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDexState {
    pub total_vault_equity: Dec,
    #[serde(default)]
    pub perps_at_open_interest_cap: Vec<String>,
    #[serde(default)]
    pub leading_vaults: Vec<LeadingVault>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LeadingVault {
    pub address: Address,
    pub name: String,
}

/// `activeAssetData`: what the account may trade on one perp, buy then sell.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetData {
    pub user: Address,
    pub coin: String,
    pub leverage: Leverage,
    /// The largest order each way at the current leverage, in the coin.
    pub max_trade_szs: [Dec; 2],
    /// The margin free each way, in USD.
    pub available_to_trade: [Dec; 2],
    #[serde(default)]
    pub mark_px: Option<Dec>,
}

/// `userHistoricalOrders`: orders and what became of them, the latest first.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalOrders {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    #[serde(rename = "orderHistory")]
    pub orders: Vec<HistoricalOrder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalOrder {
    pub order: OpenOrder,
    pub status: String,
    /// Milliseconds.
    pub status_timestamp: u64,
}
