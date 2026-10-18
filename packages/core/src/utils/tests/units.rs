//! Amounts counted as whole integers of a smaller unit: exact when typed, cut toward zero when
//! computed, and never zero, negative or past a `u64`.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_typed_amount_is_counted_exactly_and_a_computed_one_cut_toward_zero() {
    for (value, places, wanted) in [
        ("10", 6, Ok(10_000_000)),
        ("2.5", 6, Ok(2_500_000)),
        ("100", 8, Ok(10_000_000_000)),
        ("0.00000001", 8, Ok(1)),
        ("1.0000001", 6, Err(BadUnits::TooFine { value: dec("1.0000001"), places: 6 })),
        ("0", 6, Err(BadUnits::NotPositive(Dec::ZERO))),
        ("-1", 6, Err(BadUnits::NotPositive(dec("-1")))),
        ("18446744073709.551616", 6, Err(BadUnits::TooLarge(dec("18446744073709.551616")))),
    ] {
        assert_eq!(exact_units(dec(value), places), wanted, "{value}");
    }
    assert_eq!(cut_units(dec("604931.6453119999"), 6), Ok(604_931_645_311));
    assert_eq!(cut_units(dec("0.0000009"), 6), Err(BadUnits::NotPositive(dec("0.0000009"))), "dust cuts to nothing");
}
