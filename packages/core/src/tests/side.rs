use super::*;

#[test]
fn a_side_reads_and_writes_b_and_a() {
    assert_eq!(serde_json::from_str::<Side>(r#""B""#).unwrap(), Side::Buy);
    assert_eq!(serde_json::from_str::<Side>(r#""A""#).unwrap(), Side::Sell);
    assert_eq!(serde_json::to_string(&Side::Buy).unwrap(), r#""B""#);
    assert_eq!(serde_json::to_string(&Side::Sell).unwrap(), r#""A""#);
    for side in Side::ALL {
        assert_eq!(Side::parse(side.wire()), Some(side));
    }
}

#[test]
fn anything_but_b_or_a_is_refused() {
    for said in [r#""b""#, r#""Buy""#, r#""""#, "0", "true"] {
        assert!(serde_json::from_str::<Side>(said).is_err(), "{said}");
    }
    assert_eq!(Side::parse("S"), None);
    assert_eq!(Side::parse(" B"), None);
}

#[test]
fn each_side_knows_its_other_and_whether_it_buys() {
    assert!(Side::Buy.is_buy());
    assert!(!Side::Sell.is_buy());
    assert_eq!(Side::Buy.other(), Side::Sell);
    assert_eq!(Side::Sell.other(), Side::Buy);
}

#[test]
fn a_price_never_rounds_against_the_trader() {
    assert_eq!(Side::Buy.price_rounding(), Round::Down);
    assert_eq!(Side::Sell.price_rounding(), Round::Up);
}

#[test]
fn bids_sort_before_asks() {
    assert!(Side::Buy < Side::Sell);
}
