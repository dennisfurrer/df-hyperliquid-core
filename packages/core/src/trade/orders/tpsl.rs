//! Take profit and stop loss: on their own for a fixed size (`na`) or the whole position
//! (`positionTpsl`), or attached to an entry (`normalTpsl`).

use crate::trade::builder::BuilderSetting;
use crate::trade::orders::trigger::{Exit, exit_order, trigger_on};
use crate::trade::orders::{Cloid, Grouping, Order, PlaceOrder, Side, Tpsl};
use crate::trade::ticket::{Amount, Market, Refusal, at_least, routed};
use crate::utils::Dec;
use crate::utils::rounding::MINIMUM_ORDER_VALUE;

/// How much of a position a take profit or stop loss closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Covers {
    /// A fixed size.
    Size(Amount),
    /// The whole position, whatever it is when the trigger fires: size 0,
    /// grouped `positionTpsl`.
    Position,
}

/// A take profit or stop loss on its own. `side` is the side that closes:
/// a sell for a long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerIntent {
    /// The side that closes: a sell for a long.
    pub side: Side,
    /// How much of the position it closes.
    pub size: Covers,
    /// Take profit or stop loss.
    pub tpsl: Tpsl,
    /// Where it fires and how it fills.
    pub exit: Exit,
    /// The client order id; `None` for none.
    pub cloid: Option<Cloid>,
}

impl TriggerIntent {
    /// Without a client order id.
    #[must_use]
    pub const fn new(side: Side, size: Covers, tpsl: Tpsl, exit: Exit) -> TriggerIntent {
        TriggerIntent { side, size, tpsl, exit, cloid: None }
    }

    /// Sent with this client order id.
    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> TriggerIntent {
        self.cloid = Some(cloid);
        self
    }
}

/// Whether `exit` fires on the side of `entry` it should: above it for a
/// long's take profit and a short's stop, below it otherwise.
fn on_its_side(side: Side, tpsl: Tpsl, trigger: Dec, entry: Dec) -> bool {
    match (side, tpsl) {
        (Side::Buy, Tpsl::TakeProfit) | (Side::Sell, Tpsl::StopLoss) => trigger > entry,
        (Side::Buy, Tpsl::StopLoss) | (Side::Sell, Tpsl::TakeProfit) => trigger < entry,
    }
}

/// An entry's take profit and stop loss: each on its side of the entry, worth the minimum at its
/// trigger, reduce only and the other way.
pub(crate) fn attach(
    market: &Market,
    side: Side,
    size: Dec,
    entry: Dec,
    take_profit: Option<Exit>,
    stop_loss: Option<Exit>,
) -> Result<Vec<Order>, Refusal> {
    let mut exits = Vec::new();
    for (tpsl, exit) in [(Tpsl::TakeProfit, take_profit), (Tpsl::StopLoss, stop_loss)] {
        let Some(exit) = exit else { continue };
        if !on_its_side(side, tpsl, exit.trigger, entry) {
            return Err(Refusal::ExitSide { tpsl, trigger: exit.trigger, entry });
        }
        at_least(size, trigger_on(market.grid(), exit.trigger)?, MINIMUM_ORDER_VALUE)?;
        exits.push(exit_order(market, side.other(), size, tpsl, exit)?);
    }
    Ok(exits)
}

/// A take profit or stop loss on its own: reduce only, a fixed size grouped
/// `na`, or the whole position at size 0 grouped `positionTpsl`.
pub fn trigger(
    market: &Market,
    intent: &TriggerIntent,
    builder: Option<&BuilderSetting>,
) -> Result<PlaceOrder, Refusal> {
    if market.is_spot() {
        return Err(Refusal::Spot("a take profit or stop loss"));
    }
    let (size, grouping) = match intent.size {
        Covers::Position => (Dec::ZERO, Grouping::PositionTpsl),
        Covers::Size(amount) => {
            let trigger = trigger_on(market.grid(), intent.exit.trigger)?;
            let size = amount.size_on(market.grid(), trigger)?;
            at_least(size, trigger, MINIMUM_ORDER_VALUE)?;
            (size, Grouping::Na)
        }
    };
    let mut order = exit_order(market, intent.side, size, intent.tpsl, intent.exit)?;
    if let Some(cloid) = intent.cloid {
        order = order.with_cloid(cloid);
    }
    routed(PlaceOrder::one(order).grouped(grouping), builder)
}

#[cfg(test)]
#[path = "tests/tpsl.rs"]
mod tests;
