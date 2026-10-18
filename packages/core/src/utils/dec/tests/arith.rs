//! Exact sums, products and quotients, and quotients of large figures that must not overflow.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

#[test]
fn arithmetic_is_exact() {
    assert_eq!(dec("0.1").checked_add(dec("0.2")), Some(dec("0.3")));
    assert_eq!(dec("83982").checked_mul(dec("1.03")), Some(dec("86501.46")));
    assert_eq!(dec("100").checked_sub(dec("0.001")), Some(dec("99.999")));
    assert_eq!(dec("10").checked_div(dec("3"), 4, Round::Down), Some(dec("3.3333")));
    assert_eq!(dec("10").checked_div(dec("3"), 4, Round::Up), Some(dec("3.3334")));
    assert_eq!(dec("2").checked_div(dec("3"), 2, Round::Nearest), Some(dec("0.67")));
    assert_eq!(dec("1").checked_div(Dec::ZERO, 2, Round::Down), None);
}

/// Lending reserves in the hundreds of millions at ten places must not
/// overflow an i128 on the way to their ratio. Expected quotients are Python
/// `decimal`'s at 80 digits.
#[test]
fn large_figures_divide_without_overflowing() {
    let (supplied, borrowed) = (dec("612345678.9876543211"), dec("512345678.1234567891"));
    assert_eq!(borrowed.checked_div(supplied, 10, Round::Down), Some(dec("0.8366935469")));
    assert_eq!(borrowed.checked_div(supplied, 10, Round::Up), Some(dec("0.836693547")));
    assert_eq!(borrowed.checked_div(supplied, 10, Round::Nearest), Some(dec("0.836693547")));
    assert_eq!(dec("-512345678.1234567891").checked_div(supplied, 10, Round::Down), Some(dec("-0.8366935469")));
    assert_eq!(dec("-512345678.1234567891").checked_div(supplied, 10, Round::Up), Some(dec("-0.836693547")));
    assert_eq!(
        dec("498765432.1234567891").checked_div(dec("734567890.9876543213"), 10, Round::Nearest),
        Some(dec("0.6789916061"))
    );
    assert_eq!(
        dec("98765432109876543.21").checked_div(dec("0.0000000003"), 2, Round::Down),
        Some(dec("329218107032921810700000000"))
    );
    assert_eq!(dec("1").checked_div(dec("7"), 18, Round::Down), Some(dec("0.142857142857142857")));
    assert_eq!(dec("1").checked_div(dec("7"), 18, Round::Up), Some(dec("0.142857142857142858")));
}

#[test]
fn a_sum_is_exact_and_none_past_what_a_dec_holds() {
    assert_eq!(Dec::checked_sum([dec("0.1"), dec("0.2"), dec("-0.05")]), Some(dec("0.25")));
    assert_eq!(Dec::checked_sum([]), Some(Dec::ZERO), "nothing sums to zero");
    assert_eq!(Dec::checked_sum(vec![dec("1.5")].into_iter()), Some(dec("1.5")));
    let huge = Dec::new(i128::MAX, 0).unwrap();
    assert_eq!(Dec::checked_sum([huge, Dec::ONE]), None);
    assert_eq!(Dec::checked_sum([huge, Dec::ONE, Dec::ONE.negate()]), None, "an overflow on the way is not undone");
}
