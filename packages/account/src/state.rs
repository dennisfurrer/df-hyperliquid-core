//! What an account holds: spot balances, each perp dex's clearinghouse, and
//! the mode that decides whether they are one balance or several.

use std::fmt;

use hl_core::{Address, Dec, Json, USDC_INDEX, info_request, user_request};
use serde::{Deserialize, Deserializer, de};

/// `{"type":"spotClearinghouseState","user":..}`.
#[must_use]
pub fn spot_state_request(user: Address) -> String {
    user_request("spotClearinghouseState", user)
}

/// `{"type":"clearinghouseState","user":..}`: the main perp dex.
#[must_use]
pub fn perp_state_request(user: Address) -> String {
    user_request("clearinghouseState", user)
}

/// `{"type":"clearinghouseState","user":..,"dex":..}`: one HIP-3 dex, or the
/// main dex for `""`, which is asked for with no `dex` at all.
#[must_use]
pub fn perp_state_on_request(user: Address, dex: &str) -> String {
    if dex.is_empty() {
        return perp_state_request(user);
    }
    info_request("clearinghouseState", &[("user", Json::String(user.to_hex())), ("dex", Json::String(dex.to_owned()))])
}

/// `{"type":"userAbstraction","user":..}`.
#[must_use]
pub fn abstraction_request(user: Address) -> String {
    user_request("userAbstraction", user)
}

/// `spotClearinghouseState`: the authoritative holdings. On a unified or
/// portfolio-margin account every balance and hold of every dex is here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotState {
    pub balances: Vec<SpotBalance>,
    /// Portfolio margin: said on the answer itself, whatever the mode read.
    #[serde(default)]
    pub portfolio_margin_enabled: bool,
    /// Portfolio margin: the maintenance requirement over the liquidation
    /// value. Past 0.95 the account is liquidated.
    #[serde(default)]
    pub portfolio_margin_ratio: Option<Dec>,
    /// Pooled accounts: what each token, by index, has free above maintenance.
    #[serde(default)]
    pub token_to_available_after_maintenance: Vec<(u32, Dec)>,
}

/// One token's row. `total` already includes `hold`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotBalance {
    pub coin: String,
    /// The token's index, a number here, not the `NAME:0x..` a transfer names.
    /// HIP-4 rows (`+92260`, `o458`) carry none.
    #[serde(default)]
    pub token: Option<u32>,
    pub total: Dec,
    /// What open orders and margin hold. On a portfolio-margin account it can
    /// be negative (`-268.81821068` against a `total` of `0.55817531`, live
    /// 2026-10-06), so it is never subtracted as it stands: see [`SpotBalance::available`].
    pub hold: Dec,
    #[serde(default)]
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
    /// What is free to move: `total` less the larger positive hold (`hold` or
    /// `spotHold`), never below zero and never above `total`. `None` only past
    /// what a `Dec` counts.
    #[must_use]
    pub fn available(&self) -> Option<Dec> {
        let held = [Some(self.hold), self.spot_hold].into_iter().flatten().filter(|h| h.is_positive()).max();
        let free = self.total.checked_sub(held.unwrap_or(Dec::ZERO))?;
        Some(free.max(Dec::ZERO).min(self.total.max(Dec::ZERO)))
    }

    /// A HIP-4 row: a side (`+92260`, `#92260`) or a question (`o458`). Never
    /// collateral, and never a token a transfer names.
    #[must_use]
    pub fn is_outcome(&self) -> bool {
        let coin = self.coin.as_str();
        coin.starts_with(['+', '#'])
            || coin.strip_prefix('o').is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    }
}

impl SpotState {
    #[must_use]
    pub fn balance(&self, token: u32) -> Option<&SpotBalance> {
        self.balances.iter().find(|balance| balance.token == Some(token) && !balance.is_outcome())
    }

    /// Spot USDC, by index rather than name. An account that never held any
    /// has no row.
    #[must_use]
    pub fn usdc(&self) -> Option<&SpotBalance> {
        self.balance(USDC_INDEX)
    }

    /// Every row with something in it, tokens and outcome sides alike, the
    /// largest total first: what the venue says is held, which is the
    /// authoritative answer (hip4-tui's D9).
    pub fn held(&self) -> impl Iterator<Item = &SpotBalance> {
        let mut held: Vec<&SpotBalance> = self.balances.iter().filter(|row| !row.total.is_zero()).collect();
        held.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.coin.cmp(&b.coin)));
        held.into_iter()
    }

    /// [`SpotBalance::available`] of a token: nothing for a token never held,
    /// and nothing, never more, past what a `Dec` counts.
    #[must_use]
    pub fn free(&self, token: u32) -> Dec {
        self.balance(token).and_then(SpotBalance::available).unwrap_or(Dec::ZERO)
    }

    /// What a pooled account's token has free above maintenance, where the
    /// venue says.
    #[must_use]
    pub fn available_after_maintenance(&self, token: u32) -> Option<Dec> {
        self.token_to_available_after_maintenance.iter().find(|(held, _)| *held == token).map(|(_, free)| *free)
    }

    /// How near liquidation a portfolio-margin account stands.
    #[must_use]
    pub fn risk(&self) -> Risk {
        Risk::of(self.portfolio_margin_ratio)
    }
}

/// How near liquidation a portfolio-margin account stands, by its ratio
/// (atlas, from Hyperliquid's portfolio margin page: liquidatable "when
/// portfolio_margin_ratio > 0.95").
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

/// `clearinghouseState`: one perp dex's margin and positions. On a unified
/// or portfolio-margin account the figures are not a balance of their own:
/// the money is in spot. hl-core's, the shape every crate reads.
pub type PerpState = hl_core::state::Clearinghouse;

pub use hl_core::state::{AssetPosition, CumFunding, Leverage, MarginMode, MarginSummary, Position};

/// How an account's balances are kept, as `userAbstraction` answers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Abstraction {
    /// What a new account answers, never set. Standard: perp and spot apart.
    Default,
    /// Standard, set: each perp dex and spot keep their own.
    Disabled,
    /// One balance per asset, in spot.
    UnifiedAccount,
    /// Unified, and an order may borrow against HYPE and BTC.
    PortfolioMargin,
    /// Discontinued: USDC sits with the main dex's perps, and every USDC dex
    /// draws on it; everything else as standard.
    DexAbstraction,
    /// A mode this build does not know, kept as spelled. Treated as standard.
    Unknown(String),
}

impl Abstraction {
    #[must_use]
    pub fn parse(text: &str) -> Abstraction {
        match text {
            "default" => Abstraction::Default,
            "disabled" => Abstraction::Disabled,
            "unifiedAccount" => Abstraction::UnifiedAccount,
            "portfolioMargin" => Abstraction::PortfolioMargin,
            "dexAbstraction" => Abstraction::DexAbstraction,
            other => Abstraction::Unknown(other.to_owned()),
        }
    }

    #[must_use]
    pub fn wire(&self) -> &str {
        match self {
            Abstraction::Default => "default",
            Abstraction::Disabled => "disabled",
            Abstraction::UnifiedAccount => "unifiedAccount",
            Abstraction::PortfolioMargin => "portfolioMargin",
            Abstraction::DexAbstraction => "dexAbstraction",
            Abstraction::Unknown(other) => other,
        }
    }

    /// One balance, in spot: a unified or portfolio-margin account, which also
    /// refuses `spotSend`.
    #[must_use]
    pub const fn is_pooled(&self) -> bool {
        matches!(self, Abstraction::UnifiedAccount | Abstraction::PortfolioMargin)
    }

    /// Separate ledgers: each perp dex and spot keep their own.
    #[must_use]
    pub const fn is_standard(&self) -> bool {
        !self.is_pooled()
    }
}

impl fmt::Display for Abstraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// From the bare JSON string the venue answers.
impl<'de> Deserialize<'de> for Abstraction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = Abstraction;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an account mode as a string")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Abstraction, E> {
                Ok(Abstraction::parse(text))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
