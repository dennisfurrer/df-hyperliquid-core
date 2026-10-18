//! Closing a position: reduce only, the other way, `FrontendMarket` bounded past the reference. A
//! whole position closes whatever it is worth; part of one is an order like any other.

use super::*;
use crate::trade::orders::close;
use crate::trade::ticket_support::*;
use crate::utils::Dec;

#[test]
fn closing_a_position_is_a_reduce_only_market_order_the_other_way() {
    let long = Close::new(dec("0.00375"), dec("80000"), dec("0.03"));
    let short = Close::new(dec("-2"), dec("412.33"), one_percent());
    assert_eq!(
        wire(close(&btc(), &long, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"77600","s":"0.00375","r":true,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
    for (market, intent, posted) in [
        (tsla(), short, r#""b":true,"p":"416.45","s":"2","r":true"#),
        (tsla(), Close { size: Some(dec("1")), ..short }, r#""s":"1""#),
        // Four dollars of position, closed whole.
        (btc(), Close::new(dec("0.00005"), dec("80000"), one_percent()), r#""s":"0.00005""#),
    ] {
        let placed = wire(close(&market, &intent, None));
        assert!(placed.contains(posted), "{placed}");
    }
}

#[test]
fn a_close_that_is_not_a_position_s_is_refused() {
    let dust = Close::new(dec("0.00005"), dec("80000"), one_percent());
    let short = Close::new(dec("-2"), dec("412.33"), one_percent());
    for (market, intent, refused) in [
        (
            tsla(),
            Close { size: Some(dec("3")), ..short },
            Refusal::CloseTooLarge { size: dec("3"), position: dec("2") },
        ),
        (btc(), Close::new(Dec::ZERO, dec("80000"), one_percent()), Refusal::NoPosition),
        (hype(), dust, Refusal::Spot("a position")),
        (
            btc(),
            Close { size: Some(dec("0.00004")), ..dust },
            Refusal::UnderMinimum { size: dec("0.00004"), price: dec("80000"), minimum: dec("10") },
        ),
    ] {
        assert_eq!(close(&market, &intent, None), Err(refused), "{intent:?}");
    }
}
