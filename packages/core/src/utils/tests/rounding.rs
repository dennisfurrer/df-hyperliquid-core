//! Every price row was run through `formatPrice` in `@nktkas/hyperliquid`
//! 0.33.2, every size row through `formatSize`.

use crate::utils::{Asset, Dec, Round};

use super::*;
use crate::trade::orders::{Side, Tif};

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

#[test]
fn prices_land_where_the_sdk_puts_them() {
    for (price, sz_decimals, spot, wanted) in PRICES {
        let got = grid(sz_decimals, spot).tick(dec(price), Round::Down).map(Dec::to_wire);
        assert_eq!(got.as_deref(), wanted, "{price} at {sz_decimals} size decimals, spot {spot}");
    }
}

#[test]
fn sizes_are_cut_toward_zero_to_the_size_decimals() {
    for (size, sz_decimals, wanted) in [
        ("1.23456789", 5, Some("1.23456")),
        ("0.123456789", 2, Some("0.12")),
        ("100", 0, Some("100")),
        ("0.00049", 3, None),
        ("12.5", 0, Some("12")),
        ("-1", 0, None),
    ] {
        assert_eq!(Grid::perp(sz_decimals).size(dec(size)).map(Dec::to_wire).as_deref(), wanted, "{size}");
    }
}

#[test]
fn a_price_rounds_once_at_the_coarser_step() {
    // to the places (81800.5), then to five figures, would round twice and give 81801
    assert_eq!(Grid::perp(5).tick(dec("81800.45"), Round::Nearest), Some(dec("81800")));
    assert_eq!(Grid::perp(5).tick(dec("81800.5"), Round::Nearest), Some(dec("81801")));
    assert_eq!(Grid::perp(2).tick(dec("85.12345"), Round::Nearest), Some(dec("85.123")));
    assert_eq!(dec("123.456").sig_places(5), 2);
    assert_eq!(dec("0.01234567").sig_places(5), 6);
    assert_eq!(dec("123456.7").sig_places(5), 0);
}

#[test]
fn a_whole_price_skips_the_significant_figure_cap() {
    assert!(Grid::perp(0).is_on_grid(dec("123456")));
    assert!(!Grid::perp(0).is_on_grid(dec("123456.5")));
    assert!(Grid::spot(0).is_on_grid(dec("0.00001234")));
    assert!(!Grid::perp(0).is_on_grid(dec("0.00001234")));
}

#[test]
fn the_places_come_off_six_or_eight() {
    assert_eq!(Grid::perp(5).price_places(), 1);
    assert_eq!(Grid::perp(0).price_places(), 6);
    assert_eq!(Grid::perp(7).price_places(), 0);
    assert_eq!(Grid::spot(2).price_places(), 6);
}

/// A limit price never rounds against the trader: a buy down, a sell up.
#[test]
fn a_limit_price_never_rounds_against_the_trader() {
    assert_eq!(Grid::perp(5).price(dec("81462.54"), Side::Sell), Some(dec("81463")));
    assert_eq!(Grid::perp(5).price(dec("81462.54"), Side::Buy), Some(dec("81462")));
    assert_eq!(Grid::spot(0).price(dec("0.1218708"), Side::Sell), Some(dec("0.12188")));
    assert_eq!(Grid::spot(0).price(dec("0.1218708"), Side::Buy), Some(dec("0.12187")));
    let order = Grid::perp(3).limit(Asset::from_id(1), Side::Buy, dec("340.187"), dec("2.5009"), Tif::Alo).unwrap();
    assert_eq!((order.price(), order.size(), order.tif()), (dec("340.18"), dec("2.5"), Some(Tif::Alo)));
    assert_eq!(
        Grid::perp(3).limit(Asset::from_id(1), Side::Buy, dec("1"), dec("0.0004"), Tif::Gtc),
        Err(TradeError::NoSize(dec("0.0004")))
    );
}

/// The reference moved by the slippage on the side that costs more, rounded
/// back toward it, so never looser than asked.
#[test]
fn a_market_bound_is_never_looser_than_the_slippage_asked() {
    let one_percent = dec("0.01");
    assert_eq!(Grid::perp(5).market_bound(dec("85634.5"), one_percent, Side::Buy), Ok(dec("86490")));
    assert_eq!(Grid::perp(5).market_bound(dec("85634.5"), one_percent, Side::Sell), Ok(dec("84779")));
    assert_eq!(Grid::spot(2).market_bound(dec("12.542"), one_percent, Side::Buy), Ok(dec("12.667")));
    assert_eq!(Grid::perp(0).market_bound(dec("100"), Dec::ZERO, Side::Buy), Ok(dec("100")));
    assert_eq!(Grid::perp(0).market_bound(dec("100"), MAX_SLIPPAGE, Side::Sell), Ok(dec("50")));
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
