//! What the websocket says about one account: fills, orders, events, funding, the ledger, each
//! dex's clearinghouse, spot balances, TWAPs, and what it may trade. Amounts are exact [`Dec`];
//! the venue writes them as strings. Frames that open with history say so in `is_snapshot`.

use std::fmt;

use hl_core::{Address, Dec};
use serde::{Deserialize, Deserializer, de};
use serde_json::value::RawValue;

use crate::market::Side;

/// One fill of the account's, as `userFills`, `userEvents` and the TWAP feeds send it, and a TWAP
/// slice's: hl-core's, the shapes `userFills` and `userTwapSliceFills` answer.
pub use hl_core::fill::{Fill, FillLiquidation, TwapSliceFill};

/// `userFills`: the last fills first (`is_snapshot`), then each new one as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFills {
    pub user: Address,
    #[serde(default)]
    pub is_snapshot: bool,
    pub fills: Vec<Fill>,
}

/// An order as `orderUpdates` sends it: hl-core's, the shape of an `openOrders` row.
pub use hl_core::order::OpenOrder as Order;

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

/// A resting order with its trigger and type, as `openOrders` and `userHistoricalOrders` send it:
/// hl-core's, the shape of a `frontendOpenOrders` row.
pub use hl_core::order::FrontendOrder as OpenOrder;

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

/// One ledger row and what it did: hl-core's, the rows `userNonFundingLedgerUpdates` answers. A
/// type this build does not read is kept as written.
pub use hl_core::ledger::{Delta, LedgerUpdate};

/// One dex's clearinghouse, its margin and positions, a position's leverage (or what the account
/// has set on a coin) and whether it shares the account's balance: hl-core's, the shapes
/// `clearinghouseState` answers.
pub use hl_core::state::{
    AssetPosition, Clearinghouse, CumFunding, Leverage, MarginMode as Margin, MarginSummary, Position,
};

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
#[serde(rename_all = "camelCase")]
pub struct SpotState {
    pub user: Address,
    /// hl-core's, the shape `spotClearinghouseState` answers.
    #[serde(rename = "spotState")]
    pub state: hl_core::state::SpotState,
}

/// One token's balance. `total` already includes `hold`; under portfolio margin a hold may be
/// negative, room to borrow. hl-core's, the row `spotClearinghouseState` answers.
pub use hl_core::state::SpotBalance as Balance;

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

/// An order and what became of it: hl-core's, the shape of a `historicalOrders` row.
pub use hl_core::order::OrderRecord as HistoricalOrder;
