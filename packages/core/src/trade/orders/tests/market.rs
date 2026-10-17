use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;

#[test]
fn a_market_buy_is_frontend_market_bounded_above_the_mid() {
    assert_eq!(
        wire(order(&btc(), &market(Side::Buy, "0.00012"), None)),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"84840","s":"0.00012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
}

#[test]
fn a_market_sell_is_bounded_below_and_rounded_toward_the_mid() {
    assert_eq!(
        wire(order(&btc(), &market(Side::Sell, "0.00012"), None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"83161","s":"0.00012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
}

/// A market order is `FrontendMarket` at a bound, never `Gtc`, and a limit cannot be made one by
/// its TIF.

#[test]
fn a_market_order_is_never_gtc_and_a_limit_is_never_frontend_market() {
    for side in [Side::Buy, Side::Sell] {
        let placed = order(&btc(), &market(side, "0.001"), None).unwrap();
        assert!(placed.orders().iter().all(|order| order.tif() == Some(Tif::FrontendMarket)));
        assert!(!placed.wire().contains("Gtc"));
    }
    assert_eq!(order(&btc(), &limit(Side::Buy, "84000", "0.001", Tif::FrontendMarket), None), Err(Refusal::MarketTif));
    assert_eq!(
        order(&btc(), &OrderIntent::market(Side::Buy, Amount::Size(dec("0.001")), dec("84000"), dec("0.51")), None),
        Err(Refusal::Trade(TradeError::Slippage(dec("0.51"))))
    );
}
