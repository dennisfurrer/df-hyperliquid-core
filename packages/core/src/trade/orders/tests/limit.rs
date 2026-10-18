//! A limit order: on the grid, never against the trader (a buy rounds down, a sell up), its size
//! cut toward zero, and no float anywhere between what is typed and what is signed.

use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_limit_is_brought_onto_the_grid_with_the_chosen_time_in_force() {
    let mut reducing = limit(Side::Buy, "84000.7", "0.001", Tif::Alo);
    reducing.reduce_only = true;
    assert_eq!(
        wire(order(&btc(), &reducing, None)),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"84000","s":"0.001","r":true,"t":{"limit":{"tif":"Alo"}}}],"grouping":"na"}"#
    );
    assert_eq!(
        wire(order(&hype(), &limit(Side::Buy, "44.1234567", "1.239", Tif::Ioc), None)),
        r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"44.123","s":"1.23","r":false,"t":{"limit":{"tif":"Ioc"}}}],"grouping":"na"}"#,
        "spot: five figures, at most eight places less its two size decimals"
    );
    for (market, side, price, size, posted) in [
        (btc(), Side::Sell, "84000.7", "0.001", r#""p":"84001","s":"0.001""#),
        (eth(), Side::Buy, "0.3", "33.3334", r#""p":"0.3","s":"33.3334""#),
    ] {
        let placed = wire(order(&market, &limit(side, price, size, Tif::Gtc), None));
        assert!(placed.contains(posted), "{placed}");
    }
}
