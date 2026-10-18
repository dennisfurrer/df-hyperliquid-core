//! The side as the venue writes it, and which way each rounds a price.

use super::*;

#[test]
fn a_side_is_b_or_a_on_the_wire_and_never_rounds_against_the_trader() {
    let table = [(Side::Buy, "B", true, Side::Sell, Round::Down), (Side::Sell, "A", false, Side::Buy, Round::Up)];
    for (side, wire, buys, other, rounding) in table {
        assert_eq!((side.wire(), side.to_string(), Side::parse(wire)), (wire, wire.to_owned(), Some(side)));
        assert_eq!(serde_json::to_string(&side).unwrap(), format!("\"{wire}\""));
        assert_eq!(serde_json::from_str::<Side>(&format!("\"{wire}\"")).unwrap(), side);
        assert_eq!((side.is_buy(), side.other(), side.price_rounding()), (buys, other, rounding), "{wire}");
    }
    assert_eq!(Side::ALL, [Side::Buy, Side::Sell], "a bid sorts before an ask");
    assert!(Side::Buy < Side::Sell);
}

#[test]
fn anything_but_b_or_a_is_refused() {
    for said in ["b", "Buy", "", "S", " B"] {
        assert_eq!(Side::parse(said), None, "{said:?}");
        assert!(serde_json::from_str::<Side>(&format!("\"{said}\"")).is_err(), "{said:?}");
    }
    for said in ["0", "true", "null"] {
        assert!(serde_json::from_str::<Side>(said).is_err(), "{said}");
    }
}
