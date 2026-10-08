//! The complete-set scan: every market and question priced as a set.
//!
//! A complete set pays exactly one unit of the quote token at settlement:
//! both sides of a market, or every named leg of a question whose legs cover
//! every answer. Buying one at the asks for less than one unit, or selling
//! one at the bids for more, is an edge, counted in ticks. Edges are before
//! fees (D11): outcome fees bill the closing side only, at a rate the account
//! and the deployer set, so the figure here is never net.
//!
//! A question's fallback sits outside both sums and is reported beside them.
//! A field that need not cover every answer (a tournament, a deployer's own
//! list) reports its cost but never an edge.

use hl_core::OutcomeSide;

use crate::{
    book::Book,
    coin::Coin,
    market::Catalog,
    px::{Mid, OBSERVED_PER_UNIT, Px, Qty, TICKS_PER_UNIT},
    template::Family,
    time::Stamp,
};

/// The widest spread, in ticks, at which a leg's mid is read: ten cents.
pub const CREDIBLE_SPREAD: u32 = 10 * (TICKS_PER_UNIT / 100);

/// Mid units in one tick: a mid is reported finer than it can be traded.
const PER_TICK: u64 = (OBSERVED_PER_UNIT / TICKS_PER_UNIT) as u64;

/// One side's touch: its best bid and ask with what rests there, and its mid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Touch {
    pub bid: Option<(Px, Qty)>,
    pub ask: Option<(Px, Qty)>,
    pub mid: Option<Mid>,
}

impl Touch {
    /// The top of `book`, and the mid the venue reports for it.
    #[must_use]
    pub fn of(book: &Book, mid: Option<Mid>) -> Touch {
        let top = |levels: &[crate::book::Level]| levels.first().map(|level| (level.px, level.sz));
        Touch { bid: top(&book.bids), ask: top(&book.asks), mid }
    }
}

/// One leg of a set as priced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leg {
    pub coin: Coin,
    /// The side's label for a market, the leg's title for a question.
    pub label: String,
    /// An observation, not an offer.
    pub mid: Option<Mid>,
    pub bid: Option<Px>,
    pub ask: Option<Px>,
    pub bid_size: Qty,
    pub ask_size: Qty,
}

impl Leg {
    /// Both sides quoted, the ask at or above the bid, at most
    /// [`CREDIBLE_SPREAD`] apart. A crossed book is a book in flight, not the
    /// tightest market there is.
    #[must_use]
    pub fn credible(&self) -> bool {
        matches!(
            (self.bid, self.ask),
            (Some(bid), Some(ask)) if ask >= bid && ask.ticks() - bid.ticks() <= CREDIBLE_SPREAD
        )
    }

    fn priced(coin: Coin, touch: Touch) -> Leg {
        Leg {
            coin,
            label: String::new(),
            mid: touch.mid,
            bid: touch.bid.map(|(price, _)| price),
            ask: touch.ask.map(|(price, _)| price),
            bid_size: touch.bid.map_or(Qty::ZERO, |(_, size)| size),
            ask_size: touch.ask.map_or(Qty::ZERO, |(_, size)| size),
        }
    }
}

/// One direction of the trade, counted in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// The sum of the asks (buying) or the bids (selling).
    pub total: u32,
    /// What one set makes: one unit less the asks, or the bids less one
    /// unit. Before fees.
    pub before_fees: i32,
    /// The most sets the touch holds: the smallest leg's size.
    pub size: Qty,
}

impl Edge {
    /// How an edge is labelled wherever it is shown.
    pub const LABEL: &'static str = "before fees";
}

/// What a set is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Market,
    Question,
}

impl Shape {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Shape::Market => "market",
            Shape::Question => "question",
        }
    }
}

/// Which way a set is traded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// At the asks, then merged or held to settlement.
    Buy,
    /// Split from the quote token, then sold at the bids.
    Sell,
}

impl Way {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Way::Buy => "buy",
            Way::Sell => "sell",
        }
    }
}

/// One market or question priced as a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    pub shape: Shape,
    /// The market's coin (`#12100`) or the question's id.
    pub id: String,
    pub title: String,
    pub venue: Option<String>,
    pub expiry: Option<Stamp>,
    pub legs: Vec<Leg>,
    /// A question's catch-all leg, beside the sums rather than in them.
    pub fallback: Option<Leg>,
    /// Whether the legs cover every answer, so that exactly one pays.
    pub exhaustive: bool,
    /// Whether every leg is [`Leg::credible`].
    pub credible: bool,
    /// The mids in ticks, only when every leg is credible: indicative, not a
    /// price to be had.
    pub mid_sum: Option<u32>,
    pub buy: Option<Edge>,
    pub sell: Option<Edge>,
}

impl Set {
    /// The better direction with something in it, on a set that covers every
    /// answer.
    #[must_use]
    pub fn best(&self) -> Option<(Way, &Edge)> {
        best_of(self.exhaustive, self.buy.as_ref(), self.sell.as_ref())
    }

    /// How far the mids sum from one unit, in ticks.
    #[must_use]
    pub fn drift(&self) -> i32 {
        drift_of(self.exhaustive, self.mid_sum)
    }
}

fn best_of<'a>(exhaustive: bool, buy: Option<&'a Edge>, sell: Option<&'a Edge>) -> Option<(Way, &'a Edge)> {
    if !exhaustive {
        return None;
    }
    let buy = buy.filter(|edge| edge.before_fees > 0);
    let sell = sell.filter(|edge| edge.before_fees > 0);
    match (buy, sell) {
        (Some(buy), Some(sell)) if sell.before_fees > buy.before_fees => Some((Way::Sell, sell)),
        (Some(buy), _) => Some((Way::Buy, buy)),
        (None, Some(sell)) => Some((Way::Sell, sell)),
        (None, None) => None,
    }
}

fn drift_of(exhaustive: bool, mid_sum: Option<u32>) -> i32 {
    match (exhaustive, mid_sum) {
        (true, Some(sum)) => signed(sum).saturating_sub(signed(TICKS_PER_UNIT)),
        _ => 0,
    }
}

/// The question families whose named legs cover every answer.
#[must_use]
pub const fn exhaustive(kind: Family) -> bool {
    matches!(kind, Family::ContestResult | Family::PolicyRateDecision | Family::Recurring)
}

fn signed(ticks: u32) -> i32 {
    i32::try_from(ticks).unwrap_or(i32::MAX)
}

fn total(legs: &[Leg], buying: bool) -> Option<Edge> {
    let mut sum: u32 = 0;
    let mut size: Option<Qty> = None;
    for leg in legs {
        let (price, resting) = match buying {
            true => (leg.ask?, leg.ask_size),
            false => (leg.bid?, leg.bid_size),
        };
        sum = sum.saturating_add(price.ticks());
        size = Some(size.map_or(resting, |least| least.min(resting)));
    }
    let (unit, paid) = (signed(TICKS_PER_UNIT), signed(sum));
    let before_fees = match buying {
        true => unit.saturating_sub(paid),
        false => paid.saturating_sub(unit),
    };
    Some(Edge { total: sum, before_fees, size: size.unwrap_or(Qty::ZERO) })
}

fn mid_sum(legs: &[Leg]) -> Option<u32> {
    if !legs.iter().all(Leg::credible) {
        return None;
    }
    let units = legs.iter().try_fold(0u64, |sum, leg| Some(sum.saturating_add(u64::from(leg.mid?.units()))))?;
    u32::try_from(units.saturating_add(PER_TICK / 2) / PER_TICK).ok()
}

struct Spec {
    shape: Shape,
    id: String,
    title: String,
    venue: Option<String>,
    expiry: Option<Stamp>,
}

#[derive(Debug, Clone, Copy)]
struct Priced {
    exhaustive: bool,
    credible: bool,
    mid_sum: Option<u32>,
    buy: Option<Edge>,
    sell: Option<Edge>,
}

impl Priced {
    fn of(legs: &[Leg], exhaustive: bool) -> Priced {
        Priced {
            exhaustive,
            credible: legs.iter().all(Leg::credible),
            mid_sum: mid_sum(legs),
            buy: total(legs, true),
            sell: total(legs, false),
        }
    }

    /// Mispriced, drifting, or a partial field quoted anyway.
    fn worth(&self) -> bool {
        best_of(self.exhaustive, self.buy.as_ref(), self.sell.as_ref()).is_some()
            || drift_of(self.exhaustive, self.mid_sum) != 0
            || (!self.exhaustive && self.credible && self.buy.is_some())
    }
}

fn assemble(spec: Spec, prices: Priced, legs: Vec<Leg>, fallback: Option<Leg>) -> Set {
    Set {
        shape: spec.shape,
        id: spec.id,
        title: spec.title,
        venue: spec.venue,
        expiry: spec.expiry,
        legs,
        fallback,
        exhaustive: prices.exhaustive,
        credible: prices.credible,
        mid_sum: prices.mid_sum,
        buy: prices.buy,
        sell: prices.sell,
    }
}

/// Every market, and every question with at least two legs listed, priced.
#[must_use]
pub fn sets(catalog: &Catalog, touch: impl Fn(Coin) -> Touch) -> Vec<Set> {
    collect(catalog, &touch, |_| true)
}

/// The sets worth reading, best edge first, then the widest drift.
#[must_use]
pub fn gaps(catalog: &Catalog, touch: impl Fn(Coin) -> Touch) -> Vec<Set> {
    let mut found = collect(catalog, &touch, Priced::worth);
    let rank = |set: &Set| match set.best() {
        Some((_, edge)) => (1u8, edge.before_fees.unsigned_abs()),
        None => (0u8, set.drift().unsigned_abs()),
    };
    found.sort_by_key(|set| std::cmp::Reverse(rank(set)));
    found
}

/// How many sets have an edge in either direction.
#[must_use]
pub fn executable(sets: &[Set]) -> usize {
    sets.iter().filter(|set| set.best().is_some()).count()
}

fn collect(catalog: &Catalog, touch: &impl Fn(Coin) -> Touch, keep: fn(&Priced) -> bool) -> Vec<Set> {
    let leg = |coin: Coin| Leg::priced(coin, touch(coin));
    let mut found = Vec::new();
    for market in &catalog.markets {
        let mut legs = vec![leg(market.sides.yes), leg(market.sides.no)];
        let prices = Priced::of(&legs, true);
        if !keep(&prices) {
            continue;
        }
        for (one, label) in legs.iter_mut().zip([&market.sides.yes_label, &market.sides.no_label]) {
            one.label.clone_from(label);
        }
        let spec = Spec {
            shape: Shape::Market,
            id: market.market.to_string(),
            title: market.headline(),
            venue: market.venue.clone(),
            expiry: market.expiry,
        };
        found.push(assemble(spec, prices, legs, None));
    }
    let titled = |coin: Coin| {
        let market = catalog.find(coin)?;
        let mut one = leg(Coin::new(market.outcome, OutcomeSide::Yes));
        one.label.clone_from(&market.title);
        Some(one)
    };
    for question in &catalog.questions {
        let legs: Vec<Leg> = question.legs.iter().filter_map(|coin| titled(*coin)).collect();
        if legs.len() < 2 {
            continue;
        }
        let prices = Priced::of(&legs, exhaustive(question.kind));
        if !keep(&prices) {
            continue;
        }
        let spec = Spec {
            shape: Shape::Question,
            id: question.question.to_string(),
            title: question.title.clone(),
            venue: question.venue.clone(),
            expiry: question.expiry,
        };
        found.push(assemble(spec, prices, legs, question.fallback.and_then(titled)));
    }
    found
}

#[cfg(test)]
#[path = "tests/scan.rs"]
mod tests;
