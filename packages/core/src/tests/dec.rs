use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

#[test]
fn the_wire_spelling_has_no_trailing_zeros_and_no_exponent() {
    for (text, wire) in [
        ("83982.0", "83982"),
        ("0.0000125", "0.0000125"),
        ("-0.0004701178", "-0.0004701178"),
        ("00123", "123"),
        ("1.2000", "1.2"),
        (".5", "0.5"),
        ("-0.0", "0"),
        ("2460964638.0624189377", "2460964638.0624189377"),
    ] {
        assert_eq!(dec(text).to_wire(), wire, "{text}");
    }
}

#[test]
fn what_is_not_a_plain_decimal_is_refused() {
    for text in ["", ".", "1e5", "1.2.3", "abc", "--1", "1,5"] {
        assert!(Dec::parse(text).is_err(), "{text} was read");
    }
    assert!(matches!(Dec::parse("0.0000000000000000001"), Err(BadDec::TooFine(_))));
}

#[test]
fn equal_values_at_different_scales_compare_equal() {
    assert_eq!(dec("1.50"), dec("1.5"));
    assert!(dec("0.99999") < dec("1"));
    assert!(dec("-2") < dec("-1.5"));
    assert_eq!(dec("30578.5").max(dec("30578.55")), dec("30578.55"));
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

/// Lending reserves in the hundreds of millions at ten places overflowed an
/// i128 on the way to their ratio (hl-lend's report). Expected quotients are
/// Python `decimal`'s at 80 digits.
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

#[test]
fn the_venue_s_strings_read_and_floats_do_not() {
    let read: Vec<Dec> = serde_json::from_str(r#"["8155.50418728","0.0","41085992.9213469997",7]"#).unwrap();
    assert_eq!(read, [dec("8155.50418728"), Dec::ZERO, dec("41085992.9213469997"), dec("7")]);
    assert!(serde_json::from_str::<Dec>("1.5").is_err(), "a JSON fraction was a float first");
    assert!(serde_json::from_str::<Dec>(r#""1e5""#).is_err());
    assert_eq!(serde_json::to_string(&dec("12.50")).unwrap(), r#""12.5""#);
}
