//! The wire spelling, what is refused as no plain decimal, and what serde reads and writes.

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
fn the_venue_s_strings_read_and_floats_do_not() {
    let read: Vec<Dec> = serde_json::from_str(r#"["8155.50418728","0.0","41085992.9213469997",7]"#).unwrap();
    assert_eq!(read, [dec("8155.50418728"), Dec::ZERO, dec("41085992.9213469997"), dec("7")]);
    assert!(serde_json::from_str::<Dec>("1.5").is_err(), "a JSON fraction was a float first");
    assert!(serde_json::from_str::<Dec>(r#""1e5""#).is_err());
    assert_eq!(serde_json::to_string(&dec("12.50")).unwrap(), r#""12.5""#);
}
