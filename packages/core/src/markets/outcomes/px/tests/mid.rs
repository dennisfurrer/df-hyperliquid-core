//! Reported mids against the tick grid.

use super::*;

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_mid_between_two_ticks_is_no_order_price_until_rounded_onto_the_grid() {
    let cases = [
        ("0.270145", false, "0.27014", "0.27015", "0.27015"),
        ("0.612995", false, "0.61299", "0.613", "0.613"),
        ("0.27", true, "0.27", "0.27", "0.27"),
    ];
    for (text, orderable, floor, ceil, nearest) in cases {
        let mid = Mid::parse(text).unwrap();
        assert_eq!(
            (mid.to_wire(), mid.to_dec(), mid.is_orderable()),
            (text.to_owned(), Dec::parse(text).unwrap(), orderable)
        );
        assert_eq!(
            (mid.to_px_floor(), mid.to_px_ceil(), mid.to_px_nearest()),
            (px(floor), px(ceil), px(nearest)),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Mid>(&format!("\"{text}\"")).unwrap(), mid);
    }
    assert!(Px::parse("0.270145").is_err(), "a mid is not a price");
    assert!(matches!(Mid::parse("0.123456789"), Err(BadPx::TooFine(_))));
    assert!(matches!(Mid::parse("1.00000001"), Err(BadPx::OutOfRange(_))));
    assert!(matches!(Mid::parse("-0.5"), Err(BadPx::NotANumber(_))));
    assert!(matches!("x".parse::<Mid>(), Err(BadPx::NotANumber(_))));
}

#[test]
fn every_order_price_is_a_mid_on_the_grid() {
    for text in ["0.00001", "0.27", "0.26999", "0.5", "0.99999"] {
        let mid = Mid::of(px(text));
        assert!(mid.is_orderable(), "{text}");
        assert_eq!(
            (mid.to_px_nearest(), mid.to_wire(), mid.to_string()),
            (px(text), text.to_owned(), mid.to_dec().to_string())
        );
    }
    assert_eq!((Mid::of(Px::ONE), Mid::ONE.complement(), Mid::ONE.units()), (Mid::ONE, Mid::ZERO, OBSERVED_PER_UNIT));
    assert_eq!(Mid::parse("0.126535").unwrap().complement().to_wire(), "0.873465");
    assert_eq!((Mid::from_units(OBSERVED_PER_UNIT + 1), Mid::ZERO.is_zero()), (None, true));
}
