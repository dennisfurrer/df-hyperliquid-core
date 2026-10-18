//! Sizes as books print them, what they cost, and the fewest contracts worth the minimum.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn sizes_are_whole_contracts_and_say_so_when_they_are_not() {
    assert_eq!(Qty::parse("544.0"), Ok(Qty::new(544)), "as a book prints it");
    assert_eq!(Qty::parse("544").unwrap().to_wire(), "544");
    assert_eq!(Qty::parse("0.5"), Err(BadQty::Fractional("0.5".to_owned())));
    assert!(matches!(Qty::parse("x"), Err(BadQty::NotAnInteger(_))));
    assert!(matches!(Qty::parse("-1"), Err(BadQty::NotAnInteger(_))));
    assert_eq!(Qty::new(50).to_dec(), Dec::from_int(50));
    assert_eq!(Qty::from_dec(dec("50")), Ok(Qty::new(50)));
    assert_eq!(Qty::from_dec(dec("50.5")), Err(BadQty::Fractional("50.5".to_owned())));
}

#[test]
fn a_notional_is_the_size_times_the_price_in_quote_token() {
    assert_eq!(Qty::new(544).notional(px("0.27")), dec("146.88"));
    assert_eq!(Qty::new(34).notional(px("0.21349")), dec("7.25866"), "a live fill, 2026-09-24");
    assert_eq!(Qty::new(1).notional(Px::ONE), Dec::ONE);
    assert_eq!(Qty::new(0).notional(px("0.27")), Dec::ZERO);
    assert_eq!(Qty::new(u64::MAX).notional(Px::ONE), Dec::new(i128::from(u64::MAX), 0).unwrap(), "never wraps");
}

/// An order worth under 10 USDC at its own price, the spot minimum, is refused before a nonce is
/// drawn.
#[test]
fn the_ten_dollar_minimum_tracks_the_price_and_not_the_collateral() {
    let near_one = px("0.99697");
    assert_eq!(Qty::least_at(near_one), Some(Qty::new(11)));
    assert!(Qty::new(11).worth_minimum(near_one));
    assert!(!Qty::new(10).worth_minimum(near_one));
    assert_eq!(Qty::least_at(px("0.5")), Some(Qty::new(20)));
    assert_eq!(Qty::least_at(px("0.01")), Some(Qty::new(1_000)));
    assert_eq!(Qty::least_at(Px::ONE), Some(Qty::new(10)));
    assert_eq!(Qty::least_at(Px::ZERO), None, "nothing at zero is worth anything");
}

#[test]
fn an_amount_of_quote_buys_whole_contracts_toward_zero() {
    assert_eq!(Qty::spend(dec("10"), px("0.612")), Some(Qty::new(16)), "ten dollars of YES");
    assert_eq!(Qty::spend(dec("10"), px("0.5")), Some(Qty::new(20)));
    assert_eq!(Qty::spend(dec("0.4"), px("0.5")), Some(Qty::ZERO));
    assert_eq!(Qty::spend(dec("10"), Px::ZERO), None);
    assert_eq!(Qty::spend(dec("-1"), px("0.5")), None);
}
