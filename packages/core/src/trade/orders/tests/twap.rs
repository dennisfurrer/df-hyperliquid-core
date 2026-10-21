//! A TWAP: 5 minutes to a day, at least $100 at its reference (the venue's order types page), and
//! no notional without a reference to size it at.

use super::*;
use crate::trade::orders::twap;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_twap_runs_five_minutes_to_a_day_and_totals_at_least_a_hundred() {
    let at = |amount: Amount, minutes: u32| TwapIntent::new(Side::Buy, amount, minutes).at(dec("84000"));
    let sized = |size: &str| Amount::Size(dec(size));
    assert_eq!(
        wire(twap(&btc(), &at(sized("0.01"), 30))),
        r#"{"type":"twapOrder","twap":{"a":0,"b":true,"s":"0.01","r":false,"m":30,"t":false}}"#
    );
    let mut reducing = TwapIntent::new(Side::Sell, sized("1"), 5);
    reducing.reduce_only = true;
    for (market, intent, refused) in [
        (btc(), at(sized("0.01"), 4), Refusal::Trade(TradeError::TwapMinutes(4))),
        (btc(), at(sized("0.01"), 1441), Refusal::Trade(TradeError::TwapMinutes(1441))),
        (
            btc(),
            at(sized("0.001"), 30),
            Refusal::UnderMinimum { size: dec("0.001"), price: dec("84000"), minimum: MINIMUM_TWAP_VALUE },
        ),
        (btc(), TwapIntent::new(Side::Buy, Amount::Notional(dec("500")), 30), Refusal::NoReference),
        (hype(), reducing, Refusal::Spot("reduce only")),
    ] {
        assert_eq!(twap(&market, &intent), Err(refused), "{intent:?}");
    }
    let unpriced = wire(twap(&btc(), &TwapIntent::new(Side::Sell, sized("0.123456"), 5)));
    assert!(unpriced.contains(r#""b":false,"s":"0.12345""#), "a size alone is cut to the grid: {unpriced}");
}

/// A TWAP and its cancel name the market they act on, as an order does.
#[test]
fn a_twap_and_its_cancel_name_their_asset() {
    let asset = btc().asset();
    let placed = twap(&btc(), &TwapIntent::new(Side::Buy, Amount::Size(dec("0.01")), 30));
    assert_eq!(placed.map(|twap| twap.asset()), Ok(asset));
    assert_eq!(TwapCancel::new(asset, 1234).asset(), asset);
}
