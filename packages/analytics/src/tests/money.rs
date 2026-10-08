//! hip4-tui's `engine/src/tests/payout/money.rs`, on exact decimals rather than its hundred-
//! millionths.

use super::*;

#[test]
fn a_plain_amount_reads_and_writes_back_unchanged() {
    for said in ["0", "1", "12.5", "3.25", "0.000001", "1000000"] {
        assert_eq!(wire(parse(said).unwrap()), said, "{said}");
    }
}

#[test]
fn digits_past_six_places_are_dropped_rather_than_rounded_up() {
    assert_eq!(wire(parse("1.9999999").unwrap()), "1.999999");
    assert_eq!(wire(parse("0.0000009").unwrap()), "0");
    assert_eq!(wire(parse("12.3456789").unwrap()), "12.345678");
    assert_eq!(wire(parse("1.0000000000000000000009").unwrap()), "1", "past what a Dec holds, still cut");
}

#[test]
fn a_short_fraction_is_padded_rather_than_read_short() {
    assert_eq!(parse("0.5").unwrap(), parse("0.500000").unwrap());
    assert_ne!(parse("0.5").unwrap(), parse("0.05").unwrap());
}

#[test]
fn a_negative_amount_is_refused_by_name() {
    assert!(matches!(parse("-1"), Err(BadUsdc::Negative(_))));
    assert!(matches!(parse("-0.000001"), Err(BadUsdc::Negative(_))));
}

#[test]
fn anything_that_is_not_a_decimal_amount_is_refused() {
    for said in ["", "abc", "1.2.3", "1e6", "0x10", ".", "1,000", ".5"] {
        assert!(matches!(parse(said), Err(BadUsdc::NotANumber(_))), "{said}");
    }
}

#[test]
fn an_amount_too_large_to_count_is_refused_rather_than_wrapping() {
    assert!(matches!(parse("99999999999999999999"), Err(BadUsdc::OutOfRange(_))));
    assert!(parse("18446744073709.551615").is_ok(), "the most millionths a u64 counts");
    assert!(matches!(parse("18446744073709.551616"), Err(BadUsdc::OutOfRange(_))));
}

#[test]
fn whitespace_around_an_amount_is_not_part_of_it() {
    assert_eq!(parse("  12.5  ").unwrap(), parse("12.5").unwrap());
    assert_eq!(parse("+12.5").unwrap(), parse("12.5").unwrap());
}

#[test]
fn everything_read_here_is_payable_as_usdc() {
    for said in ["0.1", "1.9999999", "0.000001", "7"] {
        assert!(is_payable(parse(said).unwrap()), "{said}");
    }
    assert!(!is_payable(Dec::lit::<8>(1)), "a hundred-millionth is finer than USDC counts");
}

#[test]
fn a_total_adds_every_row_exactly() {
    let rows = ["1.5", "2.25", "0.000001"].map(|said| parse(said).unwrap());
    assert_eq!(wire(total(rows)), "3.750001");
    assert_eq!(total(Vec::new()), Dec::ZERO);
}

#[test]
fn six_places_is_the_smallest_step_there_is() {
    let one = parse("0.000001").unwrap();
    assert_eq!(one, Dec::lit::<6>(1));
    assert_eq!(micros(one), Some(1));
    assert_eq!(micros(parse("12.5").unwrap()), Some(12_500_000));
    assert_eq!(wire(one), "0.000001");
}
