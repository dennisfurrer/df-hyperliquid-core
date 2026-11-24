//! `portfolio`: the venue's own history of what an account is worth and its profit, over a day, a
//! week, a month and all time, for the whole account and its perps alone. Samples come oldest
//! first; profit is a running sum from zero at the span's first sample.

use std::fmt;

use crate::client::{ApiError, Transport, info, user_request};
use crate::utils::{Address, Dec};
use serde::{Deserialize, Deserializer, de};

/// `{"type":"portfolio","user":..}`.
#[must_use]
pub fn portfolio_request(user: Address) -> String {
    user_request("portfolio", user)
}

/// Weighs 20.
pub async fn portfolio(transport: &impl Transport, user: Address) -> Result<Portfolio, ApiError> {
    info(transport, portfolio_request(user)).await
}

/// A span the venue samples over.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Span {
    /// The last day (`day`).
    Day,
    /// The last week (`week`).
    Week,
    /// The last month (`month`).
    Month,
    /// Since the account's first sample (`allTime`).
    AllTime,
    /// The last day, perps only (`perpDay`).
    PerpDay,
    /// The last week, perps only (`perpWeek`).
    PerpWeek,
    /// The last month, perps only (`perpMonth`).
    PerpMonth,
    /// Since the account's first sample, perps only (`perpAllTime`).
    PerpAllTime,
    /// One this build does not know, kept as spelled.
    Other(String),
}

impl Span {
    /// From the venue's name: `perpWeek`. A name this build does not know is kept as
    /// [`Span::Other`].
    #[must_use]
    pub fn parse(text: &str) -> Span {
        match text {
            "day" => Span::Day,
            "week" => Span::Week,
            "month" => Span::Month,
            "allTime" => Span::AllTime,
            "perpDay" => Span::PerpDay,
            "perpWeek" => Span::PerpWeek,
            "perpMonth" => Span::PerpMonth,
            "perpAllTime" => Span::PerpAllTime,
            other => Span::Other(other.to_owned()),
        }
    }

    /// As the venue spells it: `perpWeek`.
    #[must_use]
    pub fn wire(&self) -> &str {
        match self {
            Span::Day => "day",
            Span::Week => "week",
            Span::Month => "month",
            Span::AllTime => "allTime",
            Span::PerpDay => "perpDay",
            Span::PerpWeek => "perpWeek",
            Span::PerpMonth => "perpMonth",
            Span::PerpAllTime => "perpAllTime",
            Span::Other(other) => other,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = Span;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a span's name")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Span, E> {
                Ok(Span::parse(text))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

/// One span's samples, each `(milliseconds, figure)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct History {
    /// What the account was worth at each sample (`accountValueHistory`), in USDC.
    #[serde(default)]
    pub account_value_history: Vec<(u64, Dec)>,
    /// The profit at each sample (`pnlHistory`), in USDC, a running sum from zero at the first.
    #[serde(default)]
    pub pnl_history: Vec<(u64, Dec)>,
    /// Volume traded over the span.
    #[serde(default)]
    pub vlm: Dec,
}

/// `portfolio`: every span, in the order the venue sends them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
#[non_exhaustive]
pub struct Portfolio {
    /// Each span with its samples, as `(span, samples)`.
    pub spans: Vec<(Span, History)>,
}

impl Portfolio {
    /// The samples of one span; `None` when the venue did not send it.
    #[must_use]
    pub fn span(&self, span: &Span) -> Option<&History> {
        self.spans.iter().find(|(named, _)| named == span).map(|(_, history)| history)
    }

    /// What the whole account is worth at the last sample: the venue's own
    /// sum, spot, every dex, vaults and staking.
    #[must_use]
    pub fn value(&self) -> Option<Dec> {
        self.span(&Span::Day)?.account_value_history.last().map(|point| point.1)
    }

    /// The profit over the last day.
    #[must_use]
    pub fn today(&self) -> Option<Dec> {
        self.span(&Span::Day)?.pnl_history.last().map(|point| point.1)
    }
}

/// Several accounts' series summed at the times of the longest, each account
/// taken at its last sample at or before each time, or nothing before its
/// first. A sum past what a `Dec` counts keeps what it had.
#[must_use]
pub fn summed(series: &[&[(u64, Dec)]]) -> Vec<(u64, Dec)> {
    let Some(longest) = series.iter().max_by_key(|one| one.len()) else {
        return Vec::new();
    };
    longest
        .iter()
        .map(|(at, _)| {
            let sum = series.iter().fold(Dec::ZERO, |sum, one| {
                let held = one.iter().take_while(|(when, _)| when <= at).last().map_or(Dec::ZERO, |point| point.1);
                sum.checked_add(held).unwrap_or(sum)
            });
            (*at, sum)
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/portfolio.rs"]
mod tests;
