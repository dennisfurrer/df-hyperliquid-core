use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_limit_is_brought_onto_the_grid_with_the_chosen_time_in_force() {
    let mut intent = limit(Side::Buy, "84000.7", "0.001", Tif::Alo);
    intent.reduce_only = true;
    assert_eq!(
        wire(order(&btc(), &intent, None)),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"84000","s":"0.001","r":true,"t":{"limit":{"tif":"Alo"}}}],"grouping":"na"}"#
    );
    // a sell rounds up: it never takes less than asked
    assert!(wire(order(&btc(), &limit(Side::Sell, "84000.7", "0.001", Tif::Gtc), None)).contains(r#""p":"84001""#));
}
