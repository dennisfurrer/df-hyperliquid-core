//! An entry: sized from a notional at its own price, routed through the builder at its market's
//! share of the cap, and refused here for whatever the venue would refuse.

use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;
use crate::utils::{Asset, Dec};

#[test]
fn a_notional_is_cut_to_a_size_at_the_order_s_price() {
    assert_eq!(btc().grid().least_size(dec("84000")), Some(dec("0.00012")), "ten dollars at 84000, rounded up");
    let purr = Market::spot(Asset::from_id(10_000), 0).unwrap();
    assert_eq!(purr.grid().least_size(dec("0.12564")), Some(dec("80")));
    // 100 / 84000 = 0.0011904..., cut; a market order sizes at the mid, not at its bound.
    for intent in [
        OrderIntent::limit(Side::Buy, Amount::Notional(dec("100")), dec("84000"), Tif::Gtc),
        OrderIntent::market(Side::Sell, Amount::Notional(dec("100")), dec("84000.5"), one_percent()),
    ] {
        assert!(wire(order(&btc(), &intent, None)).contains(r#""s":"0.00119""#), "{intent:?}");
    }
}

/// One flat fee for every order: over a market's cap the order is refused with the reason, and the
/// fee is never lowered.
#[test]
fn every_order_carries_the_builder_at_its_share_and_a_fee_over_the_cap_is_refused() {
    let flat = |fee: u64| BuilderSetting::flat(builder_address(), BuilderFee::from_tenths(fee));
    let perp = limit(Side::Buy, "84000", "0.001", Tif::Gtc);
    let spot = limit(Side::Buy, "44", "1", Tif::Gtc);
    let hip3 = OrderIntent::market(Side::Buy, Amount::Size(dec("1")), dec("412.33"), one_percent());
    let builder = r#""grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":"#;
    for (market, intent, setting, routed) in [
        (btc(), perp, at_88_percent(), Ok(88)),
        (hype(), spot, at_88_percent(), Ok(880)),
        (btc(), perp, flat(100), Ok(100)),
        (hype(), spot, flat(1000), Ok(1000)),
        (btc(), perp, flat(101), Err("the builder fee of 101 is over the 100 a perp order allows (0.1%)")),
        (tsla(), hip3, flat(500), Err("the builder fee of 500 is over the 100 a HIP-3 perp order allows (0.1%)")),
        (hype(), spot, flat(1001), Err("the builder fee of 1001 is over the 1000 a spot order allows (1%)")),
    ] {
        let placed = order(&market, &intent, Some(&setting));
        match routed {
            Ok(fee) => assert!(placed.unwrap().wire().ends_with(&format!("{builder}{fee}}}}}"))),
            Err(reason) => assert_eq!(placed.unwrap_err().to_string(), reason),
        }
    }
}

#[test]
fn an_order_the_venue_would_refuse_is_refused_before_signing() {
    let mut reducing = limit(Side::Buy, "44", "1", Tif::Gtc);
    reducing.reduce_only = true;
    let protected = limit(Side::Buy, "44", "1", Tif::Gtc).stop_loss(Exit::market(dec("40")));
    let at_market =
        |slippage: &str| OrderIntent::market(Side::Buy, Amount::Size(dec("0.001")), dec("84000"), dec(slippage));
    for (market, intent, refused) in [
        (
            btc(),
            limit(Side::Buy, "84000", "0.0001", Tif::Gtc),
            Refusal::UnderMinimum { size: dec("0.0001"), price: dec("84000"), minimum: dec("10") },
        ),
        (btc(), limit(Side::Buy, "84000", "0.000001", Tif::Gtc), Refusal::Trade(TradeError::NoSize(dec("0.000001")))),
        (btc(), limit(Side::Buy, "0", "1", Tif::Gtc), Refusal::NotPositive { what: "a price", value: Dec::ZERO }),
        (btc(), limit(Side::Buy, "84000", "-1", Tif::Gtc), Refusal::NotPositive { what: "a size", value: dec("-1") }),
        (btc(), limit(Side::Buy, "84000", "0.001", Tif::FrontendMarket), Refusal::MarketTif),
        (btc(), at_market("0.51"), Refusal::Trade(TradeError::Slippage(dec("0.51")))),
        (hype(), reducing, Refusal::Spot("reduce only")),
        (hype(), protected, Refusal::Spot("a take profit or stop loss")),
    ] {
        assert_eq!(order(&market, &intent, None), Err(refused), "{intent:?}");
    }
    assert_eq!(
        order(&btc(), &limit(Side::Buy, "84000", "0.0001", Tif::Gtc), None).unwrap_err().to_string(),
        "0.0001 at 84000 is worth 8.4, under the venue's minimum of 10"
    );
}
