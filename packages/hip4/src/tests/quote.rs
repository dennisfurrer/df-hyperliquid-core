//! atlas's quote check (`venues/hyperliquid/src/quote.rs`, from globe.li's
//! `quote-quality.ts`), on the live `#14730` book of 2026-09-28 and on the
//! cases its thresholds name. The live depth was summed by hand in Python's
//! `decimal`: 1417.21976 bid and 2538.00568 ask within two cents.

use hl_core::OutcomeSide;

use super::*;
use crate::{book::Level, coin::Coin, px::Qty};

const LIVE: &str = include_str!("../../fixtures/l2_book_14730_mainnet.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

fn level(price: &str, size: u64) -> Level {
    Level { px: px(price), sz: Qty::new(size), n: 1 }
}

fn book(bids: Vec<Level>, asks: Vec<Level>) -> Book {
    Book { coin: Coin::new(1473, OutcomeSide::Yes), time: 0, bids, asks }
}

#[test]
fn a_tight_deep_live_book_quotes_a_probability() {
    let live: Book = serde_json::from_str(LIVE).unwrap();
    assert_eq!(depth_within(&live, DEPTH_WINDOW), dec("3955.22544"));
    let quality = assess(&live, false);
    assert_eq!(
        quality,
        Quality::Firm { mid: Mid::parse("0.58496").unwrap(), spread: px("0.00002"), depth: dec("3955.22544") }
    );
    assert_eq!(quality.probability(), Some(Mid::parse("0.58496").unwrap()));
    assert_eq!(quality.depth(), Some(dec("3955.22544")));
}

/// atlas's own example: an eliminated player's "40.5%" is the midpoint of a
/// 0.00001 bid and a 0.81 ask.
#[test]
fn a_side_parked_at_the_boundary_is_no_market() {
    let eliminated = book(vec![level("0.00001", 5_000)], vec![level("0.81", 100)]);
    let quality = assess(&eliminated, false);
    assert_eq!(quality, Quality::Dead { mid: Some(Mid::parse("0.405005").unwrap()) });
    assert_eq!(quality.probability(), None);
    assert_eq!(
        assess(&book(vec![level("0.5", 500)], vec![level("0.99", 500)]), false),
        Quality::Dead { mid: Some(Mid::parse("0.745").unwrap()) }
    );
    assert!(matches!(assess(&book(vec![level("0.01", 500)], vec![level("0.02", 500)]), false), Quality::Dead { .. }));
    assert_eq!(assess(&book(vec![], vec![level("0.5", 500)]), false), Quality::Dead { mid: None });
}

#[test]
fn a_wide_or_shallow_book_is_a_range_not_a_point() {
    let deep = |bid: &str, ask: &str| book(vec![level(bid, 1_000)], vec![level(ask, 1_000)]);
    assert!(matches!(assess(&deep("0.40", "0.42"), false), Quality::Firm { .. }), "two cents is still tight");
    assert_eq!(
        assess(&deep("0.40", "0.42001"), false),
        Quality::Thin { bid: px("0.40"), ask: px("0.42001"), spread: px("0.02001"), depth: dec("820.01") }
    );
    let shallow = book(vec![level("0.40", 100)], vec![level("0.41", 140)]);
    assert_eq!(depth_within(&shallow, DEPTH_WINDOW), dec("97.4"));
    assert!(matches!(assess(&shallow, false), Quality::Thin { .. }), "under $100 within two cents");
    let enough = book(vec![level("0.40", 100)], vec![level("0.41", 147)]);
    assert!(matches!(assess(&enough, false), Quality::Firm { .. }), "100.27 is enough");
}

#[test]
fn only_money_near_the_touch_counts() {
    let far = book(vec![level("0.40", 10), level("0.38", 10), level("0.37999", 1_000)], vec![]);
    assert_eq!(depth_within(&far, DEPTH_WINDOW), dec("7.8"), "0.37999 is past two cents from 0.40");
}

#[test]
fn a_settled_leg_is_no_probability_of_anything() {
    let live: Book = serde_json::from_str(LIVE).unwrap();
    assert_eq!(assess(&live, true), Quality::Settled);
    assert_eq!(Quality::Settled.depth(), None);
}
