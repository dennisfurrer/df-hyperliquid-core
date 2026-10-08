//! The ticket: what a trader means, turned into the exact action to sign, or
//! the reason it cannot be sent. Pure functions over the market's rules, which
//! the caller reads from meta (`hl-perps`, `hl-hip3`, `hl-spot`) and passes in
//! as a [`Market`]. Ported from hip4-tui's `tui/src/trading/form.rs`, without
//! the form.
//!
//! The rules every function keeps:
//! - an order is worth at least [`MINIMUM_ORDER_VALUE`] in the quote, valued
//!   at its limit, at the reference for a market order, at the trigger for a
//!   take profit or stop loss; a whole position closes whatever it is worth;
//! - a price goes onto the grid never against the trader (a buy down, a sell
//!   up), a trigger to the nearest tick, a size toward zero;
//! - a market order is `FrontendMarket` at a bound `slippage` past the
//!   reference, never `Gtc` (hip4-tui's D7);
//! - a take profit or stop loss is reduce only, as the venue requires;
//! - a builder is checked against the strictest cap of the action's orders.

use hl_core::{Asset, AssetKind, Dec, Round};

use crate::{
    builder::BuilderSetting,
    error::TradeError,
    grid::{Grid, MINIMUM_ORDER_VALUE},
    leverage::{MarginMode, UpdateIsolatedMargin, UpdateLeverage},
    modify::{BatchModify, Modify, OrderRef},
    order::{Cloid, Grouping, Order, PlaceOrder, Side, Tif, Tpsl},
    orders::FrontendOrder,
    twap::Twap,
};

/// The slippage a market take profit or stop loss takes past its trigger:
/// the venue's TP/SL page gives market TP/SL orders a tolerance of 10%.
pub const TRIGGER_SLIPPAGE: Dec = match Dec::new(1, 1) {
    Some(tenth) => tenth,
    None => Dec::ZERO,
};

/// The least a TWAP may total, in the quote: the venue's order types page.
pub const MINIMUM_TWAP_VALUE: Dec = Dec::from_int(100);

/// The fewest levels a scale has.
pub const LEAST_LEVELS: u32 = 2;

/// The most levels a scale has: hip4-tui's ticket.
pub const MOST_LEVELS: u32 = 50;

/// What the ticket knows of one market, from the meta the caller read: its
/// asset id, its grid, and for a perp its leverage ceiling and whether it
/// takes cross margin. HIP-4 outcomes have a grid and a ticket of their own
/// in `hl-hip4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Market {
    asset: Asset,
    grid: Grid,
    max_leverage: Option<u32>,
    allows_cross: bool,
}

impl Market {
    /// A perp, main dex or HIP-3: its `szDecimals` and `maxLeverage` in
    /// `meta`, and whether it takes cross (not `onlyIsolated`, and no
    /// `marginMode` that forbids it). `None` for an id that names no perp.
    #[must_use]
    pub const fn perp(asset: Asset, sz_decimals: u32, max_leverage: u32, allows_cross: bool) -> Option<Market> {
        match asset.kind() {
            AssetKind::Perp | AssetKind::Hip3Perp => {
                Some(Market { asset, grid: Grid::perp(sz_decimals), max_leverage: Some(max_leverage), allows_cross })
            }
            AssetKind::Spot | AssetKind::Outcome => None,
        }
    }

    /// A spot pair, by its base token's `szDecimals` in `spotMeta`. `None`
    /// for an id that names no pair.
    #[must_use]
    pub const fn spot(asset: Asset, sz_decimals: u32) -> Option<Market> {
        match asset.kind() {
            AssetKind::Spot => {
                Some(Market { asset, grid: Grid::spot(sz_decimals), max_leverage: None, allows_cross: false })
            }
            AssetKind::Perp | AssetKind::Hip3Perp | AssetKind::Outcome => None,
        }
    }

    #[must_use]
    pub const fn asset(&self) -> Asset {
        self.asset
    }

    #[must_use]
    pub const fn grid(&self) -> Grid {
        self.grid
    }

    /// `None` on spot, which takes no leverage.
    #[must_use]
    pub const fn max_leverage(&self) -> Option<u32> {
        self.max_leverage
    }

    #[must_use]
    pub const fn allows_cross(&self) -> bool {
        self.allows_cross
    }

    /// Spot has balances, not positions: nothing that closes, protects or
    /// funds one.
    #[must_use]
    pub const fn is_spot(&self) -> bool {
        self.max_leverage.is_none()
    }
}

/// How much: a size in the base, or a notional in the quote that is cut to
/// a size at the order's price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    Size(Dec),
    Notional(Dec),
}

impl Amount {
    /// The size on `grid` at `price`, cut toward zero.
    fn size_on(self, grid: Grid, price: Dec) -> Result<Dec, Refusal> {
        let raw = match self {
            Amount::Size(size) => positive(size, "a size")?,
            Amount::Notional(notional) => positive(notional, "a notional")?
                .checked_div(price, 12, Round::Down)
                .ok_or(Refusal::Trade(TradeError::NoPrice(price)))?,
        };
        grid.size(raw).ok_or(Refusal::Trade(TradeError::NoSize(raw)))
    }
}

/// At what price an order goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Price {
    /// `FrontendMarket`, bounded `slippage` (a fraction, 0.01 for 1%) past
    /// `reference`: the mid, or else the mark.
    Market { reference: Dec, slippage: Dec },
    /// A limit at `price`, resting as `tif` says. Never `FrontendMarket`.
    Limit { price: Dec, tif: Tif },
}

/// How a take profit or stop loss fills once its trigger fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitPrice {
    /// At market, no worse than `slippage` past the trigger.
    Market { slippage: Dec },
    /// A limit at this price.
    Limit(Dec),
}

/// A take profit or stop loss: where it fires and how it fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub trigger: Dec,
    pub price: ExitPrice,
}

impl Exit {
    /// At market, within the venue's own [`TRIGGER_SLIPPAGE`].
    #[must_use]
    pub const fn market(trigger: Dec) -> Exit {
        Exit::market_within(trigger, TRIGGER_SLIPPAGE)
    }

    /// At market, within `slippage`.
    #[must_use]
    pub const fn market_within(trigger: Dec, slippage: Dec) -> Exit {
        Exit { trigger, price: ExitPrice::Market { slippage } }
    }

    /// A limit at `price` once `trigger` fires.
    #[must_use]
    pub const fn limit(trigger: Dec, price: Dec) -> Exit {
        Exit { trigger, price: ExitPrice::Limit(price) }
    }
}

/// An order to place: a market or limit entry, with its take profit and stop
/// loss when it has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderIntent {
    pub side: Side,
    pub amount: Amount,
    pub price: Price,
    pub reduce_only: bool,
    pub cloid: Option<Cloid>,
    pub take_profit: Option<Exit>,
    pub stop_loss: Option<Exit>,
}

impl OrderIntent {
    #[must_use]
    pub const fn market(side: Side, amount: Amount, reference: Dec, slippage: Dec) -> OrderIntent {
        OrderIntent::at(side, amount, Price::Market { reference, slippage })
    }

    #[must_use]
    pub const fn limit(side: Side, amount: Amount, price: Dec, tif: Tif) -> OrderIntent {
        OrderIntent::at(side, amount, Price::Limit { price, tif })
    }

    const fn at(side: Side, amount: Amount, price: Price) -> OrderIntent {
        OrderIntent { side, amount, price, reduce_only: false, cloid: None, take_profit: None, stop_loss: None }
    }

    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> OrderIntent {
        self.cloid = Some(cloid);
        self
    }

    /// Placed with the entry, the other way, once it fills.
    #[must_use]
    pub const fn take_profit(mut self, exit: Exit) -> OrderIntent {
        self.take_profit = Some(exit);
        self
    }

    /// Placed with the entry, the other way, once it fills.
    #[must_use]
    pub const fn stop_loss(mut self, exit: Exit) -> OrderIntent {
        self.stop_loss = Some(exit);
        self
    }
}

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
    pub side: Side,
    pub size: Covers,
    pub tpsl: Tpsl,
    pub exit: Exit,
    pub cloid: Option<Cloid>,
}

impl TriggerIntent {
    #[must_use]
    pub const fn new(side: Side, size: Covers, tpsl: Tpsl, exit: Exit) -> TriggerIntent {
        TriggerIntent { side, size, tpsl, exit, cloid: None }
    }

    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> TriggerIntent {
        self.cloid = Some(cloid);
        self
    }
}

/// A scale: `levels` resting orders evenly spaced from `from` to `to`, the
/// size split evenly between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    pub side: Side,
    /// The total, in the base.
    pub size: Dec,
    pub from: Dec,
    pub to: Dec,
    pub levels: u32,
    pub tif: Tif,
    pub reduce_only: bool,
}

impl Scale {
    #[must_use]
    pub const fn new(side: Side, size: Dec, from: Dec, to: Dec, levels: u32, tif: Tif) -> Scale {
        Scale { side, size, from, to, levels, tif, reduce_only: false }
    }
}

/// A TWAP: the venue slices it into an order every thirty seconds for
/// `minutes`. `reference` prices a notional and the venue's minimum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwapIntent {
    pub side: Side,
    pub amount: Amount,
    pub minutes: u32,
    pub randomize: bool,
    pub reduce_only: bool,
    pub reference: Option<Dec>,
}

impl TwapIntent {
    #[must_use]
    pub const fn new(side: Side, amount: Amount, minutes: u32) -> TwapIntent {
        TwapIntent { side, amount, minutes, randomize: false, reduce_only: false, reference: None }
    }

    /// Priced at `reference`, the mid or else the mark.
    #[must_use]
    pub const fn at(mut self, reference: Dec) -> TwapIntent {
        self.reference = Some(reference);
        self
    }
}

/// A position closed at market: all of it, or `size` of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Close {
    /// The position, signed as `clearinghouseState` writes `szi`: below
    /// zero is short.
    pub position: Dec,
    pub size: Option<Dec>,
    pub reference: Dec,
    pub slippage: Dec,
    pub cloid: Option<Cloid>,
}

impl Close {
    /// The whole `position`, bounded `slippage` past `reference`.
    #[must_use]
    pub const fn new(position: Dec, reference: Dec, slippage: Dec) -> Close {
        Close { position, size: None, reference, slippage, cloid: None }
    }

    #[must_use]
    pub const fn with_cloid(mut self, cloid: Cloid) -> Close {
        self.cloid = Some(cloid);
        self
    }
}

/// What a resting order becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A limit order's new price, size and time in force.
    Limit { price: Dec, size: Amount, tif: Tif },
    /// A take profit's or stop loss's new trigger, price and size.
    Trigger { trigger: Dec, price: ExitPrice, size: Covers },
}

/// Why the ticket sends nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error(transparent)]
    Trade(#[from] TradeError),
    #[error("{what} above zero is needed, not {value}")]
    NotPositive { what: &'static str, value: Dec },
    #[error("{size} at {price} is worth {}, under the venue's minimum of {minimum}", worth(.size, .price))]
    UnderMinimum { size: Dec, price: Dec, minimum: Dec },
    #[error("a limit is never FrontendMarket: a market order is Price::Market, bounded from the mid")]
    MarketTif,
    #[error("a scale rests on the book, so Gtc or Alo, not {}", .0.wire())]
    ScaleTif(Tif),
    #[error("a scale has {LEAST_LEVELS} to {MOST_LEVELS} levels, not {0}")]
    Levels(u32),
    #[error("a modify that does not always place is Alo or Gtc, not {}", .0.wire())]
    ModifyTif(Tif),
    #[error("spot takes no {0}")]
    Spot(&'static str),
    #[error("a notional needs a reference price to be sized at")]
    NoReference,
    #[error("a {} at {trigger} is on the wrong side of an entry at {entry}", tpsl_words(.tpsl))]
    ExitSide { tpsl: Tpsl, trigger: Dec, entry: Dec },
    #[error("there is no position here")]
    NoPosition,
    #[error("closing {size} is more than the position of {position}")]
    CloseTooLarge { size: Dec, position: Dec },
    #[error("leverage is a whole number from 1 to {most}, not {asked}")]
    Leverage { asked: u32, most: u32 },
    #[error("this market is isolated only")]
    IsolatedOnly,
    #[error("this position is cross; margin moves on isolated ones")]
    CrossPosition,
    #[error("order {0} is a trigger: change its trigger instead")]
    NotALimit(u64),
    #[error("order {0} is not a take profit or stop loss")]
    NotATrigger(u64),
    #[error("order {oid} is a {kind} trigger, which this build cannot modify")]
    UnknownTrigger { oid: u64, kind: String },
    #[error("order {0} has a size of its own; only a position's take profit or stop loss covers the whole position")]
    NotWholePosition(u64),
}

fn worth(size: &Dec, price: &Dec) -> String {
    size.checked_mul(*price).map_or_else(|| "too much to count".to_owned(), Dec::to_wire)
}

const fn tpsl_words(tpsl: &Tpsl) -> &'static str {
    match tpsl {
        Tpsl::TakeProfit => "take profit",
        Tpsl::StopLoss => "stop loss",
    }
}

fn positive(value: Dec, what: &'static str) -> Result<Dec, Refusal> {
    match value.is_positive() {
        true => Ok(value),
        false => Err(Refusal::NotPositive { what, value }),
    }
}

fn at_least(size: Dec, price: Dec, minimum: Dec) -> Result<(), Refusal> {
    match price.checked_mul(size).is_some_and(|worth| worth >= minimum) {
        true => Ok(()),
        false => Err(Refusal::UnderMinimum { size, price, minimum }),
    }
}

/// A trigger on the grid: to the nearest tick, since where it fires favours
/// neither side.
fn trigger_on(grid: Grid, trigger: Dec) -> Result<Dec, Refusal> {
    let trigger = positive(trigger, "a trigger price")?;
    grid.tick(trigger, Round::Nearest).ok_or(Refusal::Trade(TradeError::NoPrice(trigger)))
}

/// A limit on the grid for `side`, never against the trader.
fn limit_on(grid: Grid, price: Dec, side: Side, what: &'static str) -> Result<Dec, Refusal> {
    let price = positive(price, what)?;
    grid.price(price, side).ok_or(Refusal::Trade(TradeError::NoPrice(price)))
}

/// The price a trigger order carries: its bound past the trigger at market,
/// or its limit.
fn fill_price(grid: Grid, trigger: Dec, price: ExitPrice, side: Side) -> Result<(Dec, bool), Refusal> {
    match price {
        ExitPrice::Market { slippage } => Ok((grid.market_bound(trigger, slippage, side)?, true)),
        ExitPrice::Limit(price) => Ok((limit_on(grid, price, side, "a limit price")?, false)),
    }
}

/// A reduce-only trigger closing `size` on `side`.
fn exit_order(market: &Market, side: Side, size: Dec, tpsl: Tpsl, exit: Exit) -> Result<Order, Refusal> {
    let trigger = trigger_on(market.grid, exit.trigger)?;
    let (price, is_market) = fill_price(market.grid, trigger, exit.price, side)?;
    Ok(Order::trigger(market.asset, side, price, size, trigger, tpsl, is_market).reduce_only())
}

/// `orders` through the builder, held to their strictest cap.
fn routed(place: PlaceOrder, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    let Some(setting) = builder else { return Ok(place) };
    match setting.on_assets(place.orders().iter().map(Order::asset)).map_err(TradeError::from)? {
        Some(builder) => Ok(place.through(builder)),
        None => Ok(place),
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

/// An `order`: the entry, and its take profit and stop loss grouped
/// `normalTpsl` when it has them.
pub fn order(market: &Market, intent: &OrderIntent, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    let has_exits = intent.take_profit.is_some() || intent.stop_loss.is_some();
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    if market.is_spot() && has_exits {
        return Err(Refusal::Spot("a take profit or stop loss"));
    }
    let grid = market.grid;
    let side = intent.side;
    let (price, valued_at, tif) = match intent.price {
        Price::Market { reference, slippage } => {
            let reference = positive(reference, "a reference price")?;
            (grid.market_bound(reference, slippage, side)?, reference, Tif::FrontendMarket)
        }
        Price::Limit { tif: Tif::FrontendMarket, .. } => return Err(Refusal::MarketTif),
        Price::Limit { price, tif } => {
            let price = limit_on(grid, price, side, "a price")?;
            (price, price, tif)
        }
    };
    let size = intent.amount.size_on(grid, valued_at)?;
    at_least(size, valued_at, MINIMUM_ORDER_VALUE)?;
    let mut entry = match tif {
        Tif::FrontendMarket => Order::market(market.asset, side, price, size),
        tif => Order::limit(market.asset, side, price, size, tif),
    };
    if intent.reduce_only {
        entry = entry.reduce_only();
    }
    if let Some(cloid) = intent.cloid {
        entry = entry.with_cloid(cloid);
    }
    let mut orders = vec![entry];
    for (tpsl, exit) in [(Tpsl::TakeProfit, intent.take_profit), (Tpsl::StopLoss, intent.stop_loss)] {
        let Some(exit) = exit else { continue };
        if !on_its_side(side, tpsl, exit.trigger, valued_at) {
            return Err(Refusal::ExitSide { tpsl, trigger: exit.trigger, entry: valued_at });
        }
        at_least(size, trigger_on(grid, exit.trigger)?, MINIMUM_ORDER_VALUE)?;
        orders.push(exit_order(market, side.other(), size, tpsl, exit)?);
    }
    let grouping = match has_exits {
        true => Grouping::NormalTpsl,
        false => Grouping::Na,
    };
    routed(PlaceOrder::new(orders).grouped(grouping), builder)
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
            let trigger = trigger_on(market.grid, intent.exit.trigger)?;
            let size = amount.size_on(market.grid, trigger)?;
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

/// A scale: one `order` with an order a level. The last level is `to` itself.
pub fn scale(market: &Market, intent: &Scale, builder: Option<&BuilderSetting>) -> Result<PlaceOrder, Refusal> {
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    if !matches!(intent.tif, Tif::Gtc | Tif::Alo) {
        return Err(Refusal::ScaleTif(intent.tif));
    }
    let levels = intent.levels;
    if !(LEAST_LEVELS..=MOST_LEVELS).contains(&levels) {
        return Err(Refusal::Levels(levels));
    }
    let grid = market.grid;
    let total = positive(intent.size, "a size")?;
    let from = positive(intent.from, "a from price")?;
    let to = positive(intent.to, "a to price")?;
    let count = Dec::from_int(i64::from(levels));
    let raw = total.checked_div(count, 12, Round::Down).ok_or(Refusal::Trade(TradeError::NoSize(total)))?;
    let each = grid.size(raw).ok_or(Refusal::Trade(TradeError::NoSize(raw)))?;
    let span = to.checked_sub(from).ok_or(Refusal::Trade(TradeError::NoPrice(to)))?;
    let step = span
        .checked_div(Dec::from_int(i64::from(levels - 1)), 12, Round::Down)
        .ok_or(Refusal::Trade(TradeError::NoPrice(to)))?;
    let mut orders = Vec::new();
    for at in 0..levels {
        let raw = match at + 1 == levels {
            true => to,
            false => step
                .checked_mul(Dec::from_int(i64::from(at)))
                .and_then(|moved| from.checked_add(moved))
                .ok_or(Refusal::Trade(TradeError::NoPrice(from)))?,
        };
        let price = limit_on(grid, raw, intent.side, "a level's price")?;
        at_least(each, price, MINIMUM_ORDER_VALUE)?;
        let order = Order::limit(market.asset, intent.side, price, each, intent.tif);
        orders.push(match intent.reduce_only {
            true => order.reduce_only(),
            false => order,
        });
    }
    routed(PlaceOrder::new(orders), builder)
}

/// A `twapOrder`. It carries no builder: the venue gives it no field for one.
pub fn twap(market: &Market, intent: &TwapIntent) -> Result<Twap, Refusal> {
    if market.is_spot() && intent.reduce_only {
        return Err(Refusal::Spot("reduce only"));
    }
    let size = match (intent.amount, intent.reference) {
        (Amount::Notional(_), None) => return Err(Refusal::NoReference),
        (amount, Some(reference)) => amount.size_on(market.grid, positive(reference, "a reference price")?)?,
        (Amount::Size(size), None) => {
            let size = positive(size, "a size")?;
            market.grid.size(size).ok_or(Refusal::Trade(TradeError::NoSize(size)))?
        }
    };
    if let Some(reference) = intent.reference {
        at_least(size, reference, MINIMUM_TWAP_VALUE)?;
    }
    let twap = Twap::new(market.asset, intent.side, size, intent.minutes)?;
    let twap = match intent.reduce_only {
        true => twap.reduce_only(),
        false => twap,
    };
    Ok(match intent.randomize {
        true => twap.randomized(),
        false => twap,
    })
}

/// A resting order replaced in place, as `frontendOpenOrders` lists it. Its
/// side and reduce only stay; a take profit or stop loss always places, as
/// the venue requires of a trigger.
pub fn modify(market: &Market, order: &FrontendOrder, change: &Change) -> Result<BatchModify, Refusal> {
    let grid = market.grid;
    let side = order.side;
    let new = match *change {
        Change::Limit { .. } if order.is_trigger => return Err(Refusal::NotALimit(order.oid)),
        Change::Limit { tif, .. } if !matches!(tif, Tif::Alo | Tif::Gtc) => return Err(Refusal::ModifyTif(tif)),
        Change::Limit { price, size, tif } => {
            let price = limit_on(grid, price, side, "a new price")?;
            let size = size.size_on(grid, price)?;
            at_least(size, price, MINIMUM_ORDER_VALUE)?;
            Order::limit(market.asset, side, price, size, tif)
        }
        Change::Trigger { .. } if !order.is_trigger => return Err(Refusal::NotATrigger(order.oid)),
        Change::Trigger { trigger, price, size } => {
            let Some(tpsl) = order.tpsl() else {
                return Err(Refusal::UnknownTrigger { oid: order.oid, kind: order.order_type.word().to_lowercase() });
            };
            let trigger = trigger_on(grid, trigger)?;
            let size = match size {
                Covers::Position if order.sz.is_zero() => Dec::ZERO,
                Covers::Position => return Err(Refusal::NotWholePosition(order.oid)),
                Covers::Size(amount) => {
                    let size = amount.size_on(grid, trigger)?;
                    at_least(size, trigger, MINIMUM_ORDER_VALUE)?;
                    size
                }
            };
            let (price, is_market) = fill_price(grid, trigger, price, side)?;
            Order::trigger(market.asset, side, price, size, trigger, tpsl, is_market)
        }
    };
    let new = match order.reduce_only {
        true => new.reduce_only(),
        false => new,
    };
    Ok(BatchModify::new(vec![Modify::new(OrderRef::Oid(order.oid), new)]))
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
    let size = market.grid.size(wanted).ok_or(Refusal::Trade(TradeError::NoSize(wanted)))?;
    let reference = positive(intent.reference, "a reference price")?;
    let bound = market.grid.market_bound(reference, intent.slippage, side)?;
    if size != held {
        at_least(size, reference, MINIMUM_ORDER_VALUE)?;
    }
    let order = Order::market(market.asset, side, bound, size).reduce_only();
    let order = match intent.cloid {
        Some(cloid) => order.with_cloid(cloid),
        None => order,
    };
    routed(PlaceOrder::one(order), builder)
}

/// `updateLeverage`, held to the market's ceiling and margin mode.
pub fn leverage(market: &Market, mode: MarginMode, leverage: u32) -> Result<UpdateLeverage, Refusal> {
    let Some(most) = market.max_leverage else {
        return Err(Refusal::Spot("leverage"));
    };
    if leverage == 0 || leverage > most {
        return Err(Refusal::Leverage { asked: leverage, most });
    }
    if mode == MarginMode::Cross && !market.allows_cross {
        return Err(Refusal::IsolatedOnly);
    }
    Ok(UpdateLeverage::new(market.asset, mode, leverage)?)
}

/// `updateIsolatedMargin` on an isolated `position` (signed `szi`): `usdc`
/// above zero adds margin, below zero takes it out.
pub fn margin(market: &Market, position: Dec, mode: MarginMode, usdc: Dec) -> Result<UpdateIsolatedMargin, Refusal> {
    if market.is_spot() {
        return Err(Refusal::Spot("margin"));
    }
    if position.is_zero() {
        return Err(Refusal::NoPosition);
    }
    if mode == MarginMode::Cross {
        return Err(Refusal::CrossPosition);
    }
    let side = match position.is_positive() {
        true => Side::Buy,
        false => Side::Sell,
    };
    Ok(UpdateIsolatedMargin::new(market.asset, side, usdc)?)
}

#[cfg(test)]
#[path = "tests/ticket.rs"]
mod tests;
