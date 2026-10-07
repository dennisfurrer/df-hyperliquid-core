//! What an account holds: spot balances, the perp margin summary, and the
//! mode that decides whether the two are one balance or two.

use std::fmt;

use hl_core::{Address, Dec, USDC, user_request};
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

/// `{"type":"userAbstraction","user":..}`.
#[must_use]
pub fn abstraction_request(user: Address) -> String {
    user_request("userAbstraction", user)
}

/// `spotClearinghouseState`: the authoritative holdings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpotState {
    pub balances: Vec<SpotBalance>,
}

/// One token's row. `total` already includes `hold`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotBalance {
    pub coin: String,
    /// The token's index, a number here, not the `NAME:0x..` a transfer names.
    /// HIP-4 outcome rows (`coin` `+92260`) carry none.
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
    /// Portfolio margin: supplied as collateral or to lending.
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
}

impl SpotState {
    #[must_use]
    pub fn balance(&self, token: u32) -> Option<&SpotBalance> {
        self.balances.iter().find(|balance| balance.token == Some(token))
    }

    /// Spot USDC, by index rather than name. An account that never held any
    /// has no row.
    #[must_use]
    pub fn usdc(&self) -> Option<&SpotBalance> {
        self.balance(USDC.index)
    }
}

/// `clearinghouseState`, the part a dollar balance needs. Positions, cross
/// margin and the rest are left unread.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpState {
    pub margin_summary: MarginSummary,
    /// What can leave the perp balance now.
    pub withdrawable: Dec,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    pub account_value: Dec,
}

/// How an account's balances are kept, as `userAbstraction` answers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Abstraction {
    /// What a new account answers. Standard: perp and spot apart.
    Default,
    Disabled,
    /// Pooled: one balance, in spot.
    UnifiedAccount,
    PortfolioMargin,
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
            Abstraction::Unknown(other) => other,
        }
    }

    /// One balance, in spot: a unified or portfolio-margin account, which also
    /// refuses `spotSend`.
    #[must_use]
    pub const fn is_pooled(&self) -> bool {
        matches!(self, Abstraction::UnifiedAccount | Abstraction::PortfolioMargin)
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
