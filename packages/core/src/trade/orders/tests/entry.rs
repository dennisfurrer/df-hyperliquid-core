use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;
use crate::utils::{Asset, Dec};

#[test]
fn the_smallest_order_is_worth_the_venue_s_minimum() {
    assert_eq!(btc().grid().least_size(dec("84000")), Some(dec("0.00012")), "ten dollars at 84000, rounded up");
    let purr = Market::spot(Asset::from_id(10_000), 0).unwrap();
    assert_eq!(purr.grid().least_size(dec("0.12564")), Some(dec("80")));
}

#[test]
fn a_notional_is_cut_to_a_size_at_the_order_s_price() {
    let notional = OrderIntent::limit(Side::Buy, Amount::Notional(dec("100")), dec("84000"), Tif::Gtc);
    assert!(wire(order(&btc(), &notional, None)).contains(r#""s":"0.00119""#), "100 / 84000 = 0.0011904..., cut");
    let at_market = OrderIntent::market(Side::Sell, Amount::Notional(dec("100")), dec("84000.5"), one_percent());
    assert!(wire(order(&btc(), &at_market, None)).contains(r#""s":"0.00119""#), "a market order sizes at the mid");
}

#[test]
fn every_order_carries_the_builder_at_its_market_s_share_of_the_cap() {
    let builder = at_88_percent();
    let routed = order(&btc(), &limit(Side::Buy, "84000", "0.001", Tif::Gtc), Some(&builder)).unwrap();
    assert!(
        routed
            .wire()
            .ends_with(r#""grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":88}}"#),
        "{}",
        routed.wire()
    );
    let spot = order(&hype(), &limit(Side::Buy, "44", "1", Tif::Gtc), Some(&builder)).unwrap();
    assert!(spot.wire().ends_with(r#""f":880}}"#), "{}", spot.wire());
}

/// One flat fee for every order: over a market's cap the order is refused with the reason, and the
/// fee is never lowered.

#[test]
fn a_builder_fee_over_the_cap_for_its_kind_is_refused_with_the_reason() {
    let flat = |fee: u64| BuilderSetting::flat(terminal(), BuilderFee::from_tenths(fee));
    let perp = limit(Side::Buy, "84000", "0.001", Tif::Gtc);
    assert!(order(&btc(), &perp, Some(&flat(100))).is_ok(), "0.1% is the perp cap itself");
    assert_eq!(
        order(&btc(), &perp, Some(&flat(101))).unwrap_err().to_string(),
        "the builder fee of 101 is over the 100 a perp order allows (0.1%)"
    );
    let hip3 = OrderIntent::market(Side::Buy, Amount::Size(dec("1")), dec("412.33"), one_percent());
    assert_eq!(
        order(&tsla(), &hip3, Some(&flat(500))).unwrap_err().to_string(),
        "the builder fee of 500 is over the 100 a HIP-3 perp order allows (0.1%)"
    );
    let spot = limit(Side::Buy, "44", "1", Tif::Gtc);
    assert!(order(&hype(), &spot, Some(&flat(1000))).is_ok(), "spot takes up to 1%");
    assert_eq!(
        order(&hype(), &spot, Some(&flat(1001))).unwrap_err().to_string(),
        "the builder fee of 1001 is over the 1000 a spot order allows (1%)"
    );
}

#[test]
fn spot_takes_no_reduce_only_and_its_prices_keep_eight_places() {
    assert_eq!(
        wire(order(&hype(), &limit(Side::Buy, "44.1234567", "1.239", Tif::Ioc), None)),
        r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"44.123","s":"1.23","r":false,"t":{"limit":{"tif":"Ioc"}}}],"grouping":"na"}"#
    );
    let mut reducing = limit(Side::Buy, "44", "1", Tif::Gtc);
    reducing.reduce_only = true;
    // reduce only on spot is refused rather than dropped
    assert_eq!(order(&hype(), &reducing, None), Err(Refusal::Spot("reduce only")));
}

#[test]
fn an_order_under_the_venue_s_minimum_is_refused_here_first() {
    let refused = order(&btc(), &limit(Side::Buy, "84000", "0.0001", Tif::Gtc), None).unwrap_err();
    assert_eq!(refused, Refusal::UnderMinimum { size: dec("0.0001"), price: dec("84000"), minimum: dec("10") });
    assert_eq!(refused.to_string(), "0.0001 at 84000 is worth 8.4, under the venue's minimum of 10");
    assert_eq!(
        order(&btc(), &limit(Side::Buy, "84000", "0.000001", Tif::Gtc), None),
        Err(Refusal::Trade(TradeError::NoSize(dec("0.000001"))))
    );
    assert_eq!(
        order(&btc(), &limit(Side::Buy, "0", "1", Tif::Gtc), None),
        Err(Refusal::NotPositive { what: "a price", value: Dec::ZERO })
    );
}
