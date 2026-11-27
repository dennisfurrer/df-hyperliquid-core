//! The wire spelling, what is refused as no plain decimal, and what serde reads and writes.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

/// Read with an optional sign and at most one point; written with no trailing zeros and no
/// exponent. Anything else is refused.
#[test]
fn a_plain_decimal_reads_and_writes_back_without_trailing_zeros_and_nothing_else_reads() {
    for (text, wire) in [
        ("83982.0", Some("83982")),
        ("0.0000125", Some("0.0000125")),
        ("-0.0004701178", Some("-0.0004701178")),
        ("00123", Some("123")),
        ("1.2000", Some("1.2")),
        (".5", Some("0.5")),
        ("-0.0", Some("0")),
        ("2460964638.0624189377", Some("2460964638.0624189377")),
        ("", None),
        (".", None),
        ("1e5", None),
        ("1.2.3", None),
        ("abc", None),
        ("--1", None),
        ("1,5", None),
    ] {
        assert_eq!(Dec::parse(text).ok().map(Dec::to_wire).as_deref(), wire, "{text:?}");
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

/// Fields, not value: the two readers must agree on `units` and `scale`, not only on equality.
fn fields(read: Result<Dec, BadDec>) -> Result<(i128, u32), BadDec> {
    read.map(|dec| (dec.units, dec.scale))
}

/// The short reader takes only texts the long one reads, and reads them to the same fields: every
/// text of up to five characters from a small alphabet, then longer digit strings around the
/// nineteen-digit edge.
#[test]
fn the_short_reader_agrees_with_the_long_one() {
    let alphabet = b"-+.0159 x";
    let mut texts: Vec<String> = Vec::new();
    let mut layer: Vec<String> = vec![String::new()];
    for _ in 0..5 {
        layer = layer.iter().flat_map(|text| alphabet.iter().map(move |&c| format!("{text}{}", c as char))).collect();
        texts.extend(layer.iter().cloned());
    }
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..200_000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let len = 15 + (seed >> 59) as usize % 8;
        let text: String = (0..len)
            .map(|at| {
                let pick = (seed.rotate_left(at as u32 * 7) >> 32) % 13;
                b"-.012345678909"[pick as usize] as char
            })
            .collect();
        texts.push(text);
    }
    for text in &texts {
        if let Some(short) = Dec::parse_short(text.as_bytes()) {
            assert_eq!(Ok((short.units, short.scale)), fields(Dec::parse_long(text)), "{text:?}");
        }
        assert_eq!(fields(Dec::parse(text)), fields(Dec::parse_long(text)), "{text:?}");
    }
}
