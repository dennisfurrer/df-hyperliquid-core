//! A market order: `FrontendMarket`, never `Gtc`, bounded the slippage past the reference and
//! brought onto the grid toward it, so never looser than asked.

use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;

#[test]
fn a_market_order_is_frontend_market_bounded_past_the_reference_toward_it() {
    // One percent from 84000.5 is 84840.505 above and 83160.495 below: five figures, toward the mid.
    for (side, buy, bound) in [(Side::Buy, true, "84840"), (Side::Sell, false, "83161")] {
        let placed = order(&btc(), &market(side, "0.00012"), None).unwrap();
        assert_eq!(
            placed.wire(),
            format!(
                r#"{{"type":"order","orders":[{{"a":0,"b":{buy},"p":"{bound}","s":"0.00012","r":false,"t":{{"limit":{{"tif":"FrontendMarket"}}}}}}],"grouping":"na"}}"#
            )
        );
        assert!(placed.orders().iter().all(|order| order.tif() == Some(Tif::FrontendMarket) && order.is_market()));
        assert!(!placed.wire().contains("Gtc"));
    }
}
