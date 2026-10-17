use super::*;
use crate::trade::orders::close;
use crate::trade::ticket_support::*;
use crate::utils::Dec;

#[test]
fn closing_a_position_is_a_reduce_only_market_order_the_other_way() {
    let long = Close::new(dec("0.00375"), dec("80000"), dec("0.03"));
    assert_eq!(
        wire(close(&btc(), &long, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"77600","s":"0.00375","r":true,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
    let short = Close::new(dec("-2"), dec("412.33"), one_percent());
    assert!(wire(close(&tsla(), &short, None)).contains(r#""b":true,"p":"416.45","s":"2","r":true"#));
    let half = Close { size: Some(dec("1")), ..short };
    assert!(wire(close(&tsla(), &half, None)).contains(r#""s":"1""#));
    let too_much = Close { size: Some(dec("3")), ..short };
    assert_eq!(close(&tsla(), &too_much, None), Err(Refusal::CloseTooLarge { size: dec("3"), position: dec("2") }));
    assert_eq!(close(&btc(), &Close::new(Dec::ZERO, dec("80000"), one_percent()), None), Err(Refusal::NoPosition));
    assert_eq!(close(&hype(), &long, None), Err(Refusal::Spot("a position")));
}

/// A whole position closes whatever it is worth; a part of one is an order
/// like any other.

#[test]
fn a_whole_close_goes_under_the_minimum_and_a_part_does_not() {
    let dust = Close::new(dec("0.00005"), dec("80000"), one_percent());
    assert!(close(&btc(), &dust, None).is_ok(), "four dollars of position, closed whole");
    let part = Close { size: Some(dec("0.00004")), ..dust };
    assert!(matches!(close(&btc(), &part, None), Err(Refusal::UnderMinimum { .. })));
}
