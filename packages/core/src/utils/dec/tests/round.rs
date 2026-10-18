//! Rounding to places, to significant figures and to a count of a smaller unit.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

#[test]
fn rounding_goes_the_way_it_is_asked() {
    assert_eq!(dec("1.23456").round(2, Round::Down), dec("1.23"));
    assert_eq!(dec("1.23456").round(2, Round::Up), dec("1.24"));
    assert_eq!(dec("1.235").round(2, Round::Nearest), dec("1.24"));
    assert_eq!(dec("-1.235").round(2, Round::Nearest), dec("-1.24"));
    assert_eq!(dec("1.2").round(4, Round::Up), dec("1.2"));
    assert_eq!(dec("-1.239").round(2, Round::Down), dec("-1.23"), "down is toward zero");
    assert_eq!(dec("123456.7").round_sig(5, Round::Down), dec("123450"));
    assert_eq!(dec("0.001234567").round_sig(5, Round::Down), dec("0.0012345"));
    assert_eq!(dec("0.001234567").round_sig(5, Round::Up), dec("0.0012346"));
}

#[test]
fn a_chain_amount_in_millionths_is_an_integer() {
    assert_eq!(dec("12.5").units_at(6, Round::Down), Some(12_500_000));
    assert_eq!(dec("-0.0000015").units_at(6, Round::Down), Some(-1));
    assert_eq!(dec("-0.0000015").units_at(6, Round::Up), Some(-2));
}
