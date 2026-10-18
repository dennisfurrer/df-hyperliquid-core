//! The price and size grids. Every price row was run through `formatPrice` in
//! `@nktkas/hyperliquid` 0.33.2, every size row through `formatSize`.

use crate::utils::{Dec, Round};

use super::*;
use crate::markets::Side;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn grid(sz_decimals: u32, spot: bool) -> Grid {
    match spot {
        true => Grid::spot(sz_decimals),
        false => Grid::perp(sz_decimals),
    }
}

const PRICES: [(&str, u32, bool, Option<&str>); 15] = [
    ("97123.456789", 0, false, Some("97123")),
    ("1.23456789", 5, false, Some("1.2")),
    ("0.0000123456789", 0, true, Some("0.00001234")),
    ("83982.7", 5, false, Some("83982")),
    // the SDK cuts to five figures (123450); a whole price is always taken, so the nearest one wins
    ("123456.7", 0, false, Some("123456")),
    ("123456", 0, false, Some("123456")),
    ("3.14159265", 2, false, Some("3.1415")),
    ("0.001234567", 0, false, Some("0.001234")),
    ("0.12564", 0, true, Some("0.12564")),
    ("94.8049", 2, true, Some("94.804")),
    ("340.187", 3, false, Some("340.18")),
    ("30578.55", 4, false, Some("30578")),
    ("0.000001", 5, false, None),
    ("25.0000", 1, false, Some("25")),
    ("1.000009", 0, true, Some("1")),
];

/// Six places on a perp and eight on spot, less the size decimals, then five significant
/// figures unless the price is whole; rounded once, at the coarser of the two.
#[test]
fn a_price_lands_where_the_sdk_puts_it_rounded_once() {
    for (price, sz_decimals, spot, wanted) in PRICES {
        let got = grid(sz_decimals, spot).tick(dec(price), Round::Down).map(Dec::to_wire);
        assert_eq!(got.as_deref(), wanted, "{price} at {sz_decimals} size decimals, spot {spot}");
    }
    // To the places (81800.5), then to five figures, would round twice and give 81801.
    for (grid, price, wanted) in [
        (Grid::perp(5), "81800.45", "81800"),
        (Grid::perp(5), "81800.5", "81801"),
        (Grid::perp(2), "85.12345", "85.123"),
    ] {
        assert_eq!(grid.tick(dec(price), Round::Nearest), Some(dec(wanted)), "{price}");
    }
    for (price, places) in [("123.456", 2), ("0.01234567", 6), ("123456.7", 0)] {
        assert_eq!(dec(price).sig_places(5), places, "{price}");
    }
    for (grid, places) in [(Grid::perp(5), 1), (Grid::perp(0), 6), (Grid::perp(7), 0), (Grid::spot(2), 6)] {
        assert_eq!(grid.price_places(), places, "{grid:?}");
    }
    for (grid, price, on) in [
        (Grid::perp(0), "123456", true),
        (Grid::perp(0), "123456.5", false),
        (Grid::spot(0), "0.00001234", true),
        (Grid::perp(0), "0.00001234", false),
    ] {
        assert_eq!(grid.is_on_grid(dec(price)), on, "{grid:?} {price}");
    }
}

#[test]
fn a_size_is_cut_toward_zero_to_the_size_decimals() {
    for (size, sz_decimals, wanted) in [
        ("1.23456789", 5, Some("1.23456")),
        ("0.123456789", 2, Some("0.12")),
        ("100", 0, Some("100")),
        ("0.00049", 3, None),
        ("12.5", 0, Some("12")),
        ("-1", 0, None),
        ("2.5009", 3, Some("2.5")),
        ("0.0004", 3, None),
    ] {
        assert_eq!(Grid::perp(sz_decimals).size(dec(size)).map(Dec::to_wire).as_deref(), wanted, "{size}");
    }
}

/// A buy rounds down and a sell up.
#[test]
fn a_limit_price_never_rounds_against_the_trader() {
    for (grid, price, side, wanted) in [
        (Grid::perp(5), "81462.54", Side::Sell, "81463"),
        (Grid::perp(5), "81462.54", Side::Buy, "81462"),
        (Grid::spot(0), "0.1218708", Side::Sell, "0.12188"),
        (Grid::spot(0), "0.1218708", Side::Buy, "0.12187"),
        (Grid::perp(3), "340.187", Side::Buy, "340.18"),
    ] {
        assert_eq!(grid.price(dec(price), side), Some(dec(wanted)), "{price} {side:?}");
    }
}

/// The reference moved by the slippage on the side that costs more, rounded back toward it.
#[test]
fn a_market_bound_is_never_looser_than_the_slippage_asked() {
    let one_percent = dec("0.01");
    for (grid, reference, slippage, side, wanted) in [
        (Grid::perp(5), "85634.5", one_percent, Side::Buy, "86490"),
        (Grid::perp(5), "85634.5", one_percent, Side::Sell, "84779"),
        (Grid::spot(2), "12.542", one_percent, Side::Buy, "12.667"),
        (Grid::perp(0), "100", Dec::ZERO, Side::Buy, "100"),
        (Grid::perp(0), "100", MAX_SLIPPAGE, Side::Sell, "50"),
    ] {
        assert_eq!(grid.market_bound(dec(reference), slippage, side), Ok(dec(wanted)), "{reference} {side:?}");
    }
    for bad in ["0.51", "-0.01"] {
        assert_eq!(Grid::perp(0).market_bound(dec("100"), dec(bad), Side::Buy), Err(TradeError::Slippage(dec(bad))));
    }
}

#[test]
fn the_least_size_is_worth_the_venue_s_minimum() {
    assert_eq!(Grid::perp(5).least_size(dec("85634")), Some(dec("0.00012")));
    assert_eq!(Grid::spot(0).least_size(dec("0.15262")), Some(dec("66")));
    assert!(Grid::meets_minimum(dec("85634"), dec("0.00012")));
    assert!(!Grid::meets_minimum(dec("85634"), dec("0.00011")));
}
