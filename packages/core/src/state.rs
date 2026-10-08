//! What an account holds: on one perp dex, the same shape whether `clearinghouseState` answers
//! it, `subAccounts` carries it, or the socket's `clearinghouseState`, `allDexsClearinghouseState`
//! and `activeAssetData` send it; and in spot, outcome sides among the balances, the same whether
//! `spotClearinghouseState` answers it or the socket's `spotState` sends it. Each figure is the
//! venue's decimal string, read exactly.

use serde::Deserialize;

use crate::{
    address::Address,
    dec::{Dec, Round},
    network::USDC_INDEX,
    side::Side,
};

/// Places [`Position::mark`] is kept to: past a price's, so it only ever rounds a quotient that
/// does not end.
const MARK_PLACES: u32 = 12;

/// How a position is margined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarginMode {
    /// Shares the account's perp balance.
    Cross,
    /// Margined on its own.
    Isolated,
}

/// One dex's clearinghouse for one account. On a unified or portfolio-margin account the figures
/// are not a balance of their own: the money is in spot. A position the account has closed can
/// stay listed at size zero; [`Clearinghouse::positions`] leaves it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clearinghouse {
    /// Every position, cross and isolated.
    pub margin_summary: MarginSummary,
    /// The cross positions alone.
    pub cross_margin_summary: MarginSummary,
    pub cross_maintenance_margin_used: Dec,
    /// What can leave this dex's balance now.
    pub withdrawable: Dec,
    pub asset_positions: Vec<AssetPosition>,
    /// When the venue read it, milliseconds. Every recorded answer and frame says; hip4-tui's
    /// recording of a subaccount's state does not.
    #[serde(default)]
    pub time: Option<u64>,
}

impl Clearinghouse {
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

    /// [`Clearinghouse::holds_anything`], as hl-account names it.
    #[must_use]
    pub fn is_used(&self) -> bool {
        self.holds_anything()
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
    /// The unrealized profit over the margin at entry. Left out of hip4-tui's recording of a
    /// subaccount's state.
    #[serde(default)]
    pub return_on_equity: Option<Dec>,
    /// `null` where no move of the price liquidates it.
    #[serde(default)]
    pub liquidation_px: Option<Dec>,
    pub margin_used: Dec,
    /// The most this account may take on this coin now.
    pub max_leverage: u32,
    /// Left out of hip4-tui's recordings of a subaccount's state.
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

/// `activeAssetData`: an account on one perp, with or without a position there: the leverage in
/// force and what an order on each side may take. The venue counts these itself, in every
/// account mode.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAssetData {
    pub user: Address,
    pub coin: String,
    /// What an order here would be margined at.
    pub leverage: Leverage,
    /// The largest size an order may be, buy then sell, at the leverage over the mark.
    pub max_trade_szs: [Dec; 2],
    /// What an order may draw on, in the collateral, buy then sell.
    pub available_to_trade: [Dec; 2],
    pub mark_px: Dec,
}

impl ActiveAssetData {
    /// The largest size an order on `side` may be.
    #[must_use]
    pub const fn max_size(&self, side: Side) -> Dec {
        self.max_trade_szs[at(side)]
    }

    /// What an order on `side` may draw on.
    #[must_use]
    pub const fn available(&self, side: Side) -> Dec {
        self.available_to_trade[at(side)]
    }
}

/// Buy then sell, as the venue lists both sides.
const fn at(side: Side) -> usize {
    match side {
        Side::Buy => 0,
        Side::Sell => 1,
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

/// `spotClearinghouseState`: the authoritative holdings, outcome sides among them (`+14720`, with
/// no token). On a unified or portfolio-margin account every balance and hold of every dex is
/// here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotState {
    pub balances: Vec<SpotBalance>,
    /// Said on a portfolio-margin account's answer alone: `None` on every other answer and frame,
    /// where it is not said either way.
    #[serde(default)]
    pub portfolio_margin_enabled: Option<bool>,
    /// Portfolio margin: the maintenance requirement over the liquidation value. Past 0.95 the
    /// account is liquidated.
    #[serde(default)]
    pub portfolio_margin_ratio: Option<Dec>,
    /// Pooled accounts: what each token, by index, has free above maintenance; none listed on a
    /// standard account.
    #[serde(default)]
    pub token_to_available_after_maintenance: Vec<(u32, Dec)>,
}

impl SpotState {
    #[must_use]
    pub fn balance(&self, token: u32) -> Option<&SpotBalance> {
        self.balances.iter().find(|balance| balance.token == Some(token) && !balance.is_outcome())
    }

    /// Spot USDC, by index rather than name. An account that never held any has no row.
    #[must_use]
    pub fn usdc(&self) -> Option<&SpotBalance> {
        self.balance(USDC_INDEX)
    }

    /// Every row with something in it, tokens and outcome sides alike, the largest total first:
    /// what the venue says is held, which is the authoritative answer (hip4-tui's D9).
    pub fn held(&self) -> impl Iterator<Item = &SpotBalance> {
        let mut held: Vec<&SpotBalance> = self.balances.iter().filter(|row| !row.total.is_zero()).collect();
        held.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.coin.cmp(&b.coin)));
        held.into_iter()
    }

    /// [`SpotBalance::available`] of a token: nothing for a token never held, and nothing, never
    /// more, past what a `Dec` counts.
    #[must_use]
    pub fn free(&self, token: u32) -> Dec {
        self.balance(token).and_then(SpotBalance::available).unwrap_or(Dec::ZERO)
    }

    /// What a pooled account's token has free above maintenance, where the venue says.
    #[must_use]
    pub fn available_after_maintenance(&self, token: u32) -> Option<Dec> {
        self.token_to_available_after_maintenance.iter().find(|(held, _)| *held == token).map(|(_, free)| *free)
    }
}

/// One token's row. `total` already includes `hold`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotBalance {
    pub coin: String,
    /// The token's index, a number here, not the `NAME:0x..` a transfer names. HIP-4 rows
    /// (`+92260`, `o458`) carry none.
    #[serde(default)]
    pub token: Option<u32>,
    pub total: Dec,
    /// What open orders and margin hold. On a portfolio-margin account it can be negative
    /// (`-268.81821068` against a `total` of `0.55817531`, live 2026-10-06), so it is never
    /// subtracted as it stands: see [`SpotBalance::available`].
    pub hold: Dec,
    /// What it cost.
    pub entry_ntl: Dec,
    /// Portfolio margin: what spot orders hold.
    #[serde(default)]
    pub spot_hold: Option<Dec>,
    /// Portfolio margin: owed on this token.
    #[serde(default)]
    pub borrowed: Option<Dec>,
    /// Portfolio margin: supplied as collateral or to lending. Part of `total`.
    #[serde(default)]
    pub supplied: Option<Dec>,
    /// Portfolio margin: the loan-to-value this token counts at.
    #[serde(default)]
    pub ltv: Option<Dec>,
}

impl SpotBalance {
    /// What is free to move: `total` less the larger positive hold (`hold` or `spotHold`), never
    /// below zero and never above `total`. `None` only past what a `Dec` counts.
    #[must_use]
    pub fn available(&self) -> Option<Dec> {
        let held = [Some(self.hold), self.spot_hold].into_iter().flatten().filter(|h| h.is_positive()).max();
        let free = self.total.checked_sub(held.unwrap_or(Dec::ZERO))?;
        Some(free.max(Dec::ZERO).min(self.total.max(Dec::ZERO)))
    }

    /// A HIP-4 row: a side (`+92260`, `#92260`) or a question (`o458`). Never collateral, and
    /// never a token a transfer names.
    #[must_use]
    pub fn is_outcome(&self) -> bool {
        let coin = self.coin.as_str();
        coin.starts_with(['+', '#'])
            || coin.strip_prefix('o').is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    }
}
