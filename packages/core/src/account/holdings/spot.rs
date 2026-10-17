//! `spotClearinghouseState`: an account's spot balances, outcome sides among them, the same shape
//! the socket sends, and how near liquidation a portfolio-margin account stands. On a unified or
//! portfolio-margin account every balance and hold of every dex is here.

use serde::Deserialize;

use crate::client::network::USDC_INDEX;
use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::{Address, Dec};

/// `{"type":"spotClearinghouseState","user":..}`.
#[must_use]
pub fn spot_state_request(user: Address) -> String {
    user_request("spotClearinghouseState", user)
}

/// Weighs 2.
pub async fn spot_state(transport: &impl Transport, user: Address) -> Result<SpotState, ApiError> {
    info(transport, spot_state_request(user)).await
}

/// `spotClearinghouseState`: the authoritative holdings, outcome sides among them (`+14720`, with
/// no token).
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

    /// Every row with something in it, tokens and outcome sides alike, the largest total first: what
    /// the venue says is held, which is the authoritative answer.
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

    /// How near liquidation a portfolio-margin account stands, by its ratio.
    #[must_use]
    pub fn risk(&self) -> Risk {
        Risk::of(self.portfolio_margin_ratio)
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
    /// (`-268.81821068` against a `total` of `0.55817531`), so it is never subtracted as it stands:
    /// see [`SpotBalance::available`].
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

/// How near liquidation a portfolio-margin account stands, by its ratio; the venue liquidates past
/// 0.95.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Risk {
    #[default]
    Safe,
    /// From 0.85: worth a warning.
    Near,
    /// Past 0.95.
    Liquidatable,
}

impl Risk {
    #[must_use]
    pub fn of(ratio: Option<Dec>) -> Risk {
        let (near, liquidatable) = (Dec::lit::<2>(85), Dec::lit::<2>(95));
        match ratio {
            Some(ratio) if ratio > liquidatable => Risk::Liquidatable,
            Some(ratio) if ratio >= near => Risk::Near,
            _ => Risk::Safe,
        }
    }
}

#[cfg(test)]
#[path = "tests/spot.rs"]
mod tests;
