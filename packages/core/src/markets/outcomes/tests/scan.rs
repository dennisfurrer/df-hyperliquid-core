//! Sums worked by hand in cents, and a catalogue read from `outcomeMeta` priced from books.

use std::collections::BTreeMap;

use super::*;
use crate::markets::outcomes::book::{Book, Level};
use crate::markets::outcomes::meta::{Outcome, OutcomeMeta, SideSpec};

/// Hundredths of a unit, in ticks.
fn ticks(cents: u32) -> u32 {
    cents * (TICKS_PER_UNIT / 100)
}

fn price(cents: u32) -> Px {
    Px::from_ticks(ticks(cents)).unwrap()
}

fn quoted(bid: u32, ask: u32, size: u64) -> Leg {
    Leg {
        coin: Coin::parse("#12100").unwrap(),
        label: "Yes".to_owned(),
        mid: Some(Mid::of(price((bid + ask) / 2))),
        bid: Some(price(bid)),
        ask: Some(price(ask)),
        bid_size: Qty::new(size),
        ask_size: Qty::new(size),
    }
}

fn built(exhaustive: bool, legs: Vec<Leg>, fallback: Option<Leg>) -> Set {
    let spec = Spec {
        shape: Shape::Question,
        id: "200".to_owned(),
        title: "a question".to_owned(),
        venue: None,
        expiry: None,
    };
    let prices = Priced::of(&legs, exhaustive);
    assemble(spec, prices, legs, fallback)
}

#[test]
fn buying_a_set_below_one_is_the_edge() {
    let legs = vec![quoted(38, 40, 100), quoted(53, 55, 60)];
    let buy = total(&legs, true).unwrap();
    assert_eq!(buy.total, ticks(95));
    assert_eq!(buy.before_fees, 5_000, "five cents, in ticks");
    assert_eq!(buy.size, Qty::new(60), "the smallest leg governs");
}

#[test]
fn selling_a_set_above_one_is_the_other_side() {
    let legs = vec![quoted(60, 62, 10), quoted(45, 47, 25)];
    let sell = total(&legs, false).unwrap();
    assert_eq!(sell.total, ticks(105));
    assert_eq!(sell.before_fees, 5_000);
    assert_eq!(sell.size, Qty::new(10));
}

#[test]
fn an_edge_says_it_is_before_fees() {
    assert_eq!(Edge::LABEL, "before fees", "outcome fees bill the closing side only");
}

#[test]
fn a_leg_with_no_book_makes_the_set_unpriceable() {
    let mut legs = vec![quoted(38, 40, 100), quoted(53, 55, 60)];
    legs[1].ask = None;
    assert!(total(&legs, true).is_none());
    assert!(total(&legs, false).is_some(), "the other side still prices");
}

#[test]
fn a_wide_or_missing_quote_is_not_a_price() {
    let mut leg = quoted(38, 40, 100);
    assert!(leg.credible());
    leg.ask = Some(price(91));
    assert!(!leg.credible(), "a bid of 0.38 against an ask of 0.91 means nothing");
    leg.ask = Some(price(48));
    assert!(leg.credible(), "ten cents is credible");
    leg.ask = Some(price(49));
    assert!(!leg.credible());
    leg.ask = None;
    assert!(!leg.credible());
    assert_eq!(CREDIBLE_SPREAD, ticks(10));
}

#[test]
fn a_crossed_book_is_not_the_tightest_market_there_is() {
    let mut leg = quoted(40, 42, 100);
    leg.ask = Some(price(40));
    assert!(leg.credible(), "a touching book is still a book");
    leg.ask = Some(price(39));
    assert!(!leg.credible(), "an ask under the bid is a book in flight, not a zero spread");
    leg.ask = Some(price(1));
    assert!(!leg.credible(), "and the wider it crosses the less it means");
}

#[test]
fn a_set_holding_a_crossed_or_unpriced_leg_reports_no_mid_sum() {
    let crossed =
        Leg { mid: Some(Mid::of(price(50))), bid: Some(price(60)), ask: Some(price(40)), ..quoted(38, 40, 100) };
    let set = built(true, vec![quoted(38, 40, 100), crossed], None);
    assert!(!set.credible, "one crossed leg is one leg not to be read");
    assert_eq!((set.mid_sum, set.drift()), (None, 0));
    let wide = Leg { mid: Some(Mid::of(price(50))), bid: Some(price(1)), ask: Some(price(99)), ..quoted(38, 40, 100) };
    let set = built(true, vec![quoted(38, 40, 100), wide], None);
    assert_eq!((set.mid_sum, set.drift()), (None, 0), "an untraded leg reports 0.5 and would invent a gap");
}

#[test]
fn a_fairly_priced_set_offers_nothing() {
    let set = Set {
        mid_sum: Some(TICKS_PER_UNIT),
        buy: Some(Edge { total: ticks(101), before_fees: -1_000, size: Qty::new(1) }),
        sell: Some(Edge { total: ticks(99), before_fees: -1_000, size: Qty::new(1) }),
        ..built(true, vec![], None)
    };
    assert!(set.best().is_none());
    assert_eq!(set.drift(), 0);
}

#[test]
fn a_partial_set_never_reports_an_edge() {
    let field = vec![quoted(20, 25, 50), quoted(20, 25, 50)];
    let set = built(exhaustive(Family::TournamentWinner), field.clone(), None);
    assert!(set.buy.is_some(), "the cost is still reported");
    assert!(set.best().is_none(), "but it is not an edge");
    assert_eq!(set.drift(), 0);
    let contest = built(exhaustive(Family::ContestResult), field, None);
    assert!(contest.best().is_some(), "home, draw and away do cover it");
    assert!(exhaustive(Family::Recurring) && exhaustive(Family::PolicyRateDecision));
    assert!(!exhaustive(Family::Named), "a deployer's own question may leave an answer out");
}

#[test]
fn a_fallback_leg_sits_outside_both_sums() {
    let set = built(true, vec![quoted(38, 40, 100), quoted(53, 55, 60)], Some(quoted(18, 20, 5)));
    assert_eq!(set.buy.unwrap().total, ticks(95));
    assert_eq!(set.sell.unwrap().total, ticks(91));
    assert_eq!(set.mid_sum, Some(ticks(93)));
    assert_eq!(set.fallback.unwrap().ask, Some(price(20)), "reported beside the sums");
    assert_eq!(set.legs.len(), 2, "the fallback is not one of the legs");
}

#[test]
fn the_better_direction_wins() {
    let edge = |before_fees: i32| Edge { total: TICKS_PER_UNIT, before_fees, size: Qty::new(1) };
    let mut set = Set { buy: Some(edge(2_000)), sell: Some(edge(7_000)), ..built(true, vec![], None) };
    assert_eq!(set.best().map(|(way, _)| way), Some(Way::Sell));
    set.sell = Some(edge(1_000));
    assert_eq!(set.best().map(|(way, _)| way), Some(Way::Buy));
    assert_eq!((Way::Buy.label(), Way::Sell.label(), Shape::Question.label()), ("buy", "sell", "question"));
}

fn touch(bid: u32, ask: u32, size: u64) -> Book {
    let level = |cents, size| Level { px: price(cents), sz: Qty::new(size), n: 1 };
    Book {
        coin: Coin::new(1210, OutcomeSide::Yes),
        time: 0,
        bids: vec![level(bid, size)],
        asks: vec![level(ask, size)],
    }
}

fn venue() -> Catalog {
    let meta = OutcomeMeta {
        outcomes: vec![Outcome {
            outcome: 1210,
            name: "template:binaryPrice4".to_owned(),
            description: "perp:BTC|threshold:100000|time:20261001-0000".to_owned(),
            side_specs: vec![SideSpec { name: "Yes".to_owned() }, SideSpec { name: "No".to_owned() }],
            quote_token: None,
            venue: Some("out".to_owned()),
            deployer_fee_scale: None,
        }],
        ..OutcomeMeta::default()
    };
    Catalog::build(&meta, 1_788_998_400_000)
}

fn quotes() -> BTreeMap<Coin, Touch> {
    let yes = Coin::new(1210, OutcomeSide::Yes);
    let no = Coin::new(1210, OutcomeSide::No);
    BTreeMap::from([
        (yes, Touch::of(&touch(38, 40, 100), Some(Mid::of(price(39))))),
        (no, Touch::of(&touch(53, 55, 60), Some(Mid::of(price(54))))),
    ])
}

#[test]
fn a_market_quoted_under_one_unit_reads_as_a_gap() {
    let (catalog, quotes) = (venue(), quotes());
    let read = |coin: Coin| quotes.get(&coin).copied().unwrap_or_default();
    let found = sets(&catalog, read);
    assert_eq!(found.len(), 1);
    let set = &found[0];
    assert_eq!((set.shape, set.id.as_str(), set.venue.as_deref()), (Shape::Market, "#12100", Some("out")));
    assert_eq!(set.title, "BTC above 100000 at 2026-10-01 00:00Z");
    assert_eq!(set.legs.iter().map(|leg| leg.label.as_str()).collect::<Vec<_>>(), ["Yes", "No"]);
    assert!(set.credible && set.exhaustive);
    assert_eq!(set.best().map(|(way, edge)| (way, edge.before_fees, edge.size)), Some((Way::Buy, 5_000, Qty::new(60))));
    assert_eq!(set.mid_sum, Some(ticks(93)));
    assert_eq!(set.drift(), -7_000);
    assert_eq!(executable(&found), 1);
    assert_eq!(gaps(&catalog, read).len(), 1);
}

#[test]
fn a_market_nobody_quotes_is_not_a_gap() {
    let catalog = venue();
    let found = sets(&catalog, |_| Touch::default());
    assert_eq!(found.len(), 1, "it is still a set");
    assert!(found[0].buy.is_none() && found[0].sell.is_none());
    assert_eq!(executable(&found), 0);
    assert!(gaps(&catalog, |_| Touch::default()).is_empty(), "and not an opportunity");
}
