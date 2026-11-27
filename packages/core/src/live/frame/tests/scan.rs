//! The cursor reads only what serde_json reads the same way, and declines everything else.

use super::*;

#[test]
fn an_integer_reads_only_as_json_writes_it() {
    for (text, read) in [
        ("0", Some(0)),
        ("7,", Some(7)),
        ("1791480444439}", Some(1_791_480_444_439)),
        ("18446744073709551615", Some(u64::MAX)),
        ("18446744073709551616", None),
        ("01", None),
        ("-1", None),
        ("1.0", None),
        ("1e3", None),
        ("", None),
        ("\"1\"", None),
    ] {
        assert_eq!(Scan::new(text).uint(), read, "{text:?}");
    }
}

#[test]
fn a_string_reads_only_without_escapes_or_control_characters() {
    for (text, read) in [
        (r#""BTC""#, Some("BTC")),
        (r#""""#, Some("")),
        (r#""xyz:XYZ100","#, Some("xyz:XYZ100")),
        ("\"#14720\"", Some("#14720")),
        ("\"\u{e9}t\u{e9}\"", Some("\u{e9}t\u{e9}")),
        (r#""a\"b""#, None),
        (r#""a\nb""#, None),
        ("\"a\nb\"", None),
        (r#""open"#, None),
        ("BTC", None),
    ] {
        assert_eq!(Scan::new(text).string(), read, "{text:?}");
    }
}

#[test]
fn literals_read_in_order_and_the_end_is_seen() {
    let mut scan = Scan::new(r#"{"px":"1.50"}"#);
    assert!(!scan.eat(r#"{"sz":"#));
    assert!(scan.eat(r#"{"px":"#));
    assert_eq!(scan.dec().map(|dec| dec.to_wire()).as_deref(), Some("1.5"));
    assert_eq!(scan.peek(), Some(b'}'));
    assert!(!scan.done());
    assert!(scan.eat("}"));
    assert!(scan.done());
    assert_eq!(scan.peek(), None);
}
