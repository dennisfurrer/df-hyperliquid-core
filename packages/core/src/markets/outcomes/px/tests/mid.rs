//! Reported mids against the tick grid.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn px(text: &str) -> Px {
    Px::parse(text).unwrap()
}

#[test]
fn a_mid_is_not_an_orderable_price() {
    let mid = Mid::parse("0.270145").unwrap();
    assert_eq!(mid.to_wire(), "0.270145");
    assert!(!mid.is_orderable());
    assert!(Px::parse("0.270145").is_err());
    assert_eq!(mid.to_px_floor().to_wire(), "0.27014");
    assert_eq!(mid.to_px_ceil().to_wire(), "0.27015");
    assert_eq!(mid.to_px_nearest().to_wire(), "0.27015");
    assert_eq!(mid.to_dec(), dec("0.270145"));
    assert!(matches!(Mid::parse("0.123456789"), Err(BadPx::TooFine(_))));
}

#[test]
fn every_orderable_price_is_an_observable_one() {
    for text in ["0.00001", "0.27", "0.26999", "0.5", "0.99999"] {
        let price = px(text);
        let mid = Mid::of(price);
        assert!(mid.is_orderable(), "{text}");
        assert_eq!(mid.to_px_nearest(), price, "{text}");
        assert_eq!(mid.to_wire(), text, "{text}");
    }
    assert_eq!(Mid::of(Px::ONE), Mid::ONE);
    assert_eq!(Mid::ONE.complement(), Mid::ZERO);
    assert_eq!(Mid::parse("0.126535").unwrap().complement().to_wire(), "0.873465");
}
