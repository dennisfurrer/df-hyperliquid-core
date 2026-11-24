//! Closing a position at market: reduce only, the other way, `FrontendMarket`.

use crate::trade::builder::BuilderSetting;
use crate::trade::error::TradeError;
use crate::trade::orders::{Cloid, Order, PlaceOrder, Side};
use crate::trade::ticket::{Market, Refusal, at_least, positive, routed};
use crate::utils::Dec;
use crate::utils::rounding::MINIMUM_ORDER_VALUE;

/// A position closed at market: all of it, or `size` of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Close {
    /// The position, signed as `clearinghouseState` writes `szi`: below
    /// zero is short.
    pub position: Dec,
    /// How much to close, in the base asset; `None` closes the whole position.
    pub size: Option<Dec>,
    /// The price the bound is measured from: the mid, or else the mark.
    pub reference: Dec,
    /// How far past `reference` the close may fill, a fraction: 0.01 is 1%.
    pub slippage: Dec,
    /// The client order id; `None` for none.
    pub cloid: Option<Cloid>,
}

impl Close {
    /// The whole `position`, bounded `slippage` past `reference`.
    #[must_use]
    pub const fn new(position: Dec, reference: Dec, slippage: Dec) -> Close {
        Close { position, size: None, reference, slippage, cloid: None }
    }

    /// Sent with this client order id.
    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> Close {
        self.cloid = Some(cloid);
        self
    }
}

/// A position closed at market: reduce only, the other way, `FrontendMarket`.
/// A whole position closes whatever it is worth; part of one is held to the
/// minimum as any order is.
pub fn close(market: &Market, intent: &Close, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    if market.is_spot() {
        return Err(Refusal::Spot("a position"));
    }
    if intent.position.is_zero() {
        return Err(Refusal::NoPosition);
    }
    let side = match intent.position.is_positive() {
        true => Side::Sell,
        false => Side::Buy,
    };
    let held = intent.position.abs();
    let wanted = match intent.size {
        None => held,
        Some(size) if positive(size, "a size")? > held => {
            return Err(Refusal::CloseTooLarge { size, position: held });
        }
        Some(size) => size,
    };
    let size = market.grid().size(wanted).ok_or(Refusal::Trade(TradeError::NoSize(wanted)))?;
    let reference = positive(intent.reference, "a reference price")?;
    let bound = market.grid().market_bound(reference, intent.slippage, side)?;
    if size != held {
        at_least(size, reference, MINIMUM_ORDER_VALUE)?;
    }
    let order = Order::market(market.asset(), side, bound, size).reduce_only();
    let order = match intent.cloid {
        Some(cloid) => order.with_cloid(cloid),
        None => order,
    };
    routed(PlaceOrder::one(order), builder)
}

#[cfg(test)]
#[path = "tests/close.rs"]
mod tests;
