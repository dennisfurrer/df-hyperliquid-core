//! The docs' BTC table (trading/margin-tiers): 40x up to 150M of notional
//! and 20x past it, as `meta` lists it under table 56.

use crate::utils::{Asset, Dec};

use super::*;
use crate::trade::error::TradeError;

fn million(count: i64) -> Dec {
    Dec::from_int(count).checked_mul(Dec::from_int(1_000_000)).unwrap()
}

fn btc() -> MarginTable {
    serde_json::from_str(
        r#"{"description":"tiered 40x","marginTiers":[{"lowerBound":"0.0","maxLeverage":40},{"lowerBound":"150000000.0","maxLeverage":20}]}"#,
    )
    .unwrap()
}

#[test]
fn a_notional_falls_in_the_highest_tier_it_reaches() {
    let table = btc();
    assert_eq!(table.description, "tiered 40x");
    assert_eq!(
        table.margin_tiers,
        [
            MarginTier { lower_bound: Dec::ZERO, max_leverage: 40 },
            MarginTier { lower_bound: million(150), max_leverage: 20 }
        ]
    );
    for (notional, most) in
        [(Dec::ZERO, 40), (million(150).checked_sub(Dec::ONE).unwrap(), 40), (million(150), 20), (million(-200), 20)]
    {
        assert_eq!(
            table.max_leverage_at(notional),
            Some(most),
            "{notional}: a bound is its tier's lower, a short counts its size"
        );
    }
    assert_eq!(MarginTable::flat(5).max_leverage_at(million(10_000)), Some(5));
    assert_eq!(table.check("BTC", 40, million(1)), Ok(()));
    assert_eq!(table.check("BTC", 20, million(200)), Ok(()));
    let over = table.check("BTC", 25, million(-200)).unwrap_err();
    assert_eq!(over, LeverageError::OverTier { name: "BTC".to_owned(), notional: million(200), max: 20, asked: 25 });
    assert_eq!(over.to_string(), "BTC allows at most 20x at a notional of 200000000, not 25x");
}

#[test]
fn an_id_under_fifty_is_flat_and_one_past_it_is_listed() {
    let listed = [(56, btc())];
    assert_eq!(MarginTable::by_id(0, &listed), None, "no perp is on table 0");
    assert_eq!(MarginTable::by_id(20, &listed), Some(MarginTable::flat(20)));
    assert_eq!(MarginTable::by_id(FLAT_TABLES_BELOW - 1, &[]), Some(MarginTable::flat(49)));
    assert_eq!(MarginTable::by_id(56, &listed), Some(btc()));
    assert_eq!(MarginTable::by_id(57, &listed), None, "an id past 50 the meta does not list");
}

#[test]
fn leverage_is_held_to_what_the_perp_allows() {
    let asset = Asset::perp(0).unwrap();
    for (name, max, cross, mode, asked, expected) in [
        ("BTC", 40, true, MarginMode::Cross, 40, Ok((40, MarginMode::Cross))),
        (
            "BTC",
            40,
            true,
            MarginMode::Cross,
            41,
            Err(LeverageError::TooHigh { name: "BTC".to_owned(), max: 40, asked: 41 }),
        ),
        ("HPOS", 3, false, MarginMode::Cross, 2, Err(LeverageError::NoCross("HPOS".to_owned()))),
        ("HPOS", 3, false, MarginMode::Isolated, 2, Ok((2, MarginMode::Isolated))),
        ("BTC", 40, true, MarginMode::Isolated, 0, Err(LeverageError::Trade(TradeError::NoLeverage))),
    ] {
        let set = checked_leverage(asset, name, max, cross, mode, asked);
        assert_eq!(set.map(|set| (set.leverage(), set.mode())), expected, "{name} {asked}x");
    }
}
