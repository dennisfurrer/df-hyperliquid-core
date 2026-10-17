//! `clearinghouseState`: one perp dex's margin and every position on it, the same shape whether
//! `/info` answers it, `subAccounts` carries it or the socket sends it, each figure read exactly.
//! On a unified or portfolio-margin account the figures are not a balance of their own: the money
//! is in spot.

use serde::Deserialize;

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::{Dex, Side};
use crate::utils::{Address, Dec, Json, Round};

/// Places [`Position::mark`] is kept to: past a price's, so it only ever rounds a quotient that
/// does not end.
const MARK_PLACES: u32 = 12;

/// `{"type":"clearinghouseState","user":..}`, with `dex` for a HIP-3 dex.
#[must_use]
pub fn clearinghouse_state_request(user: Address, dex: Dex<'_>) -> String {
    let mut fields = vec![("user", Json::String(user.to_hex()))];
    if let Some(name) = dex.name() {
        fields.push(("dex", Json::String(name.to_owned())));
    }
    info_request("clearinghouseState", &fields)
}

/// One dex's clearinghouse for one account. Weighs 2.
pub async fn clearinghouse_state(
    transport: &impl Transport,
    user: Address,
    dex: Dex<'_>,
) -> Result<ClearinghouseState, ApiError> {
    info(transport, clearinghouse_state_request(user, dex)).await
}

/// How a position is margined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    /// Shares the account's perp balance.
    Cross,
    /// Margined on its own.
    Isolated,
}

/// One dex's clearinghouse for one account. A position the account has closed can stay listed at
/// size zero; [`ClearinghouseState::positions`] leaves it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearinghouseState {
    /// Every position, cross and isolated.
    pub margin_summary: MarginSummary,
    /// The cross positions alone.
    pub cross_margin_summary: MarginSummary,
    pub cross_maintenance_margin_used: Dec,
    /// What can leave this dex's balance now.
    pub withdrawable: Dec,
    pub asset_positions: Vec<AssetPosition>,
    /// When the venue read it, in milliseconds; not always sent.
    #[serde(default)]
    pub time: Option<u64>,
}

impl ClearinghouseState {
    /// Every open position: the listed ones of size zero left out.
    pub fn positions(&self) -> impl Iterator<Item = &Position> {
        self.asset_positions.iter().map(|entry| &entry.position).filter(|position| !position.szi.is_zero())
    }

    /// An open position by its coin: `BTC`, or `xyz:TSLA` on a HIP-3 dex.
    #[must_use]
    pub fn position(&self, coin: &str) -> Option<&Position> {
        self.positions().find(|position| position.coin == coin)
    }

    /// Whether there is margin or a position here: a dex worth reading again.
    #[must_use]
    pub fn holds_anything(&self) -> bool {
        !self.margin_summary.account_value.is_zero() || self.positions().next().is_some()
    }
}

/// What a set of positions is worth and uses, in the dex's collateral.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    pub account_value: Dec,
    /// The positions' notional, every one counted as positive.
    pub total_ntl_pos: Dec,
    /// The collateral before the positions' value: the account value less what the positions
    /// are worth.
    pub total_raw_usd: Dec,
    pub total_margin_used: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetPosition {
    /// `oneWay`: the venue nets a coin's longs and shorts into one position.
    #[serde(rename = "type")]
    pub kind: String,
    pub position: Position,
}

/// One position.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// `BTC` on the main dex, `xyz:XYZ100` on a HIP-3 one.
    pub coin: String,
    /// Signed: negative is short.
    pub szi: Dec,
    pub leverage: Leverage,
    /// The average entry; `null` on a listed position of size zero.
    #[serde(default)]
    pub entry_px: Option<Dec>,
    /// The size at the mark, positive either way.
    pub position_value: Dec,
    pub unrealized_pnl: Dec,
    /// The unrealized profit over the margin at entry; not always sent.
    #[serde(default)]
    pub return_on_equity: Option<Dec>,
    /// `null` where no move of the price liquidates it.
    #[serde(default)]
    pub liquidation_px: Option<Dec>,
    pub margin_used: Dec,
    /// The most this account may take on this coin now.
    pub max_leverage: u32,
    /// Funding on the position; not always sent.
    #[serde(default)]
    pub cum_funding: Option<CumFunding>,
}

impl Position {
    #[must_use]
    pub const fn is_long(&self) -> bool {
        self.szi.is_positive()
    }

    #[must_use]
    pub const fn is_short(&self) -> bool {
        self.szi.is_negative()
    }

    /// The size, positive either way.
    #[must_use]
    pub const fn size(&self) -> Dec {
        self.szi.abs()
    }

    /// The side an order closing it takes: a sell for a long, a buy for a short; `None` at
    /// size zero.
    #[must_use]
    pub const fn closing_side(&self) -> Option<Side> {
        if self.szi.is_positive() {
            Some(Side::Sell)
        } else if self.szi.is_negative() {
            Some(Side::Buy)
        } else {
            None
        }
    }

    /// The mark the venue valued it at: its value over its size. `None` at size zero.
    #[must_use]
    pub fn mark(&self) -> Option<Dec> {
        self.position_value.checked_div(self.size(), MARK_PLACES, Round::Nearest)
    }
}

/// How a position is margined, and at what leverage; or what an account has set on a coin
/// (`activeAssetData`). An isolated one always carries its `rawUsd`: one without it is refused,
/// never read as none put up.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Leverage {
    Cross {
        value: u32,
    },
    /// `rawUsd` is the collateral put up less the notional bought (so negative for a long):
    /// what the position uses is its value plus it.
    Isolated {
        value: u32,
        #[serde(rename = "rawUsd")]
        raw_usd: Dec,
    },
}

impl Leverage {
    #[must_use]
    pub const fn value(&self) -> u32 {
        match self {
            Leverage::Cross { value } | Leverage::Isolated { value, .. } => *value,
        }
    }

    #[must_use]
    pub const fn mode(&self) -> MarginMode {
        match self {
            Leverage::Cross { .. } => MarginMode::Cross,
            Leverage::Isolated { .. } => MarginMode::Isolated,
        }
    }

    /// An isolated position's `rawUsd`.
    #[must_use]
    pub const fn raw_usd(&self) -> Option<Dec> {
        match self {
            Leverage::Cross { .. } => None,
            Leverage::Isolated { raw_usd, .. } => Some(*raw_usd),
        }
    }
}

/// Funding on one position: positive is paid, negative received.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CumFunding {
    /// Since the account first held the coin.
    pub all_time: Dec,
    /// Since this position opened.
    pub since_open: Dec,
    /// Since its size last changed.
    pub since_change: Dec,
}

#[cfg(test)]
#[path = "tests/perps.rs"]
mod tests;
