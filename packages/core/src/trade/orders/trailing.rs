//! `trailingStop`: `{type, asset, isBuy, sz, reduceOnly, retracement, activationPx}`, keys in the
//! order the exchange endpoint docs write them. The stop follows the best price the market reaches
//! and fires when the price falls back from it by the retracement, a percent or a fixed amount.
//! With an activation price it starts to follow only once the market reaches that price.
//! `activationPx` is `null` when there is none. The venue answers with the order's id.

use crate::account::{ActionKind, Signers};
use crate::trade::orders::Side;
use crate::utils::sign::{L1Action, Map, Value};
use crate::utils::{Asset, Dec};

/// How far the price may fall back from its best before the stop fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retracement {
    /// A percent of the best price: `1.5` is 1.5%, posted as `{"pct":"1.5%"}`.
    Percent(Dec),
    /// A fixed amount in the quote, posted as `{"px":"2.5"}`.
    Price(Dec),
}

impl Retracement {
    /// `{"pct":"1.5%"}` or `{"px":"2.5"}`.
    #[must_use]
    pub fn wire(self) -> Map {
        match self {
            Retracement::Percent(percent) => Map::new().put("pct", format!("{}%", percent.to_wire())),
            Retracement::Price(px) => Map::new().put("px", px.to_wire()),
        }
    }

    const fn figure(self) -> Dec {
        match self {
            Retracement::Percent(figure) | Retracement::Price(figure) => figure,
        }
    }
}

/// Why a trailing stop is not one the venue would take.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadTrailingStop {
    /// The size is not positive; the size given.
    #[error("a trailing stop's size is positive, not {0}")]
    Size(Dec),
    /// The retracement is not positive, or a percent is 100 or more; the figure given.
    #[error("a retracement is positive, and a percent is under 100: {0}")]
    Retracement(Dec),
    /// The activation price is not positive; the price given.
    #[error("an activation price is positive, not {0}")]
    Activation(Dec),
}

/// A stop that trails the best price by its [`Retracement`]. Size and prices go on the wire
/// exactly as given, so put them on the asset's grid first ([`crate::utils::Grid`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailingStop {
    asset: Asset,
    side: Side,
    size: Dec,
    reduce_only: bool,
    retracement: Retracement,
    activation: Option<Dec>,
}

impl TrailingStop {
    /// L1: an agent may sign it for its master.
    pub const KEY: Signers = ActionKind::TrailingStop.signers();

    /// `size` of `asset`, trading `side` when the price falls back from its best by `retracement`.
    pub fn new(asset: Asset, side: Side, size: Dec, retracement: Retracement) -> Result<TrailingStop, BadTrailingStop> {
        if !size.is_positive() {
            return Err(BadTrailingStop::Size(size));
        }
        let figure = retracement.figure();
        let too_far = matches!(retracement, Retracement::Percent(_)) && figure >= Dec::from_int(100);
        if !figure.is_positive() || too_far {
            return Err(BadTrailingStop::Retracement(figure));
        }
        Ok(TrailingStop { asset, side, size, reduce_only: false, retracement, activation: None })
    }

    /// Starts to trail only once the market reaches `px`.
    pub fn activated_at(mut self, px: Dec) -> Result<TrailingStop, BadTrailingStop> {
        if !px.is_positive() {
            return Err(BadTrailingStop::Activation(px));
        }
        self.activation = Some(px);
        Ok(self)
    }

    /// Only ever shrinks the position.
    #[must_use]
    pub const fn reduce_only(mut self) -> TrailingStop {
        self.reduce_only = true;
        self
    }

    /// The asset it trades.
    #[must_use]
    pub const fn asset(&self) -> Asset {
        self.asset
    }

    /// Which way it trades when it fires.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// How much it trades when it fires.
    #[must_use]
    pub const fn size(&self) -> Dec {
        self.size
    }

    /// Whether it only ever shrinks the position.
    #[must_use]
    pub const fn is_reduce_only(&self) -> bool {
        self.reduce_only
    }

    /// How far the price may fall back from its best before it fires.
    #[must_use]
    pub const fn retracement(&self) -> Retracement {
        self.retracement
    }

    /// The price the market must reach before it starts to trail; `None` to trail at once.
    #[must_use]
    pub const fn activation(&self) -> Option<Dec> {
        self.activation
    }
}

impl L1Action for TrailingStop {
    fn action(&self) -> Map {
        let activation = match self.activation {
            Some(px) => Value::from(px.to_wire()),
            None => Value::Nil,
        };
        Map::new()
            .put("type", "trailingStop")
            .put("asset", self.asset)
            .put("isBuy", self.side.is_buy())
            .put("sz", self.size.to_wire())
            .put("reduceOnly", self.reduce_only)
            .put("retracement", self.retracement.wire())
            .put("activationPx", activation)
    }
}

#[cfg(test)]
#[path = "tests/trailing.rs"]
mod tests;
