//! What an account holds: spot balances, each perp dex's clearinghouse, and
//! the mode that decides whether they are one balance or several.

use std::fmt;

use hl_core::{Address, Dec, Json, info_request, user_request};
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

/// `spotClearinghouseState` and one token's row: hl-core's, the shapes the socket sends.
pub use hl_core::state::{SpotBalance, SpotState};

/// How near liquidation a spot state's portfolio-margin account stands: atlas's thresholds, read
/// here and not in hl-core, where the state is.
pub trait SpotRisk {
    fn risk(&self) -> Risk;
}

impl SpotRisk for SpotState {
    fn risk(&self) -> Risk {
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
