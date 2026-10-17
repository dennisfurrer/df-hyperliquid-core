//! The side of an order, a fill, a trade or a book level: `B` and `A` on the wire, one type
//! wherever the venue writes it.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::utils::dec::Round;

/// `B` a buy or a bid, `A` a sell or an ask. On a trade it is the aggressor's
/// side, on a fill the account's own, on a resting order the side of the book
/// it rests on. A bid sorts before an ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Buy, Side::Sell];

    /// `B` or `A`, exactly as the venue writes it.
    #[must_use]
    pub fn parse(said: &str) -> Option<Side> {
        match said {
            "B" => Some(Side::Buy),
            "A" => Some(Side::Sell),
            _ => None,
        }
    }

    /// `B` or `A`.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Side::Buy => "B",
            Side::Sell => "A",
        }
    }

    #[must_use]
    pub const fn is_buy(self) -> bool {
        matches!(self, Side::Buy)
    }

    #[must_use]
    pub const fn other(self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    /// How a price for this side rounds onto the grid, never against the trader: a buy rounds
    /// down, a sell up.
    #[must_use]
    pub const fn price_rounding(self) -> Round {
        match self {
            Side::Buy => Round::Down,
            Side::Sell => Round::Up,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

impl Serialize for Side {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire())
    }
}

impl<'de> Deserialize<'de> for Side {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let said = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        Side::parse(&said).ok_or_else(|| de::Error::custom(format!("`{said}` is not a side; expected B or A")))
    }
}

#[cfg(test)]
#[path = "tests/side.rs"]
mod tests;
