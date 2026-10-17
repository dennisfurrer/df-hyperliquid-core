use super::*;
use crate::trade::orders::twap;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_twap_runs_between_five_minutes_and_a_day() {
    let mut intent = TwapIntent::new(Side::Buy, Amount::Size(dec("0.01")), 30).at(dec("84000.5"));
    assert_eq!(
        wire(twap(&btc(), &intent)),
        r#"{"type":"twapOrder","twap":{"a":0,"b":true,"s":"0.01","r":false,"m":30,"t":false}}"#
    );
    intent.minutes = 4;
    assert_eq!(twap(&btc(), &intent), Err(Refusal::Trade(TradeError::TwapMinutes(4))));
}

/// The venue's order types page: a TWAP totals at least $100.

#[test]
fn a_twap_worth_less_than_a_hundred_is_refused() {
    let small = TwapIntent::new(Side::Buy, Amount::Size(dec("0.001")), 30).at(dec("84000"));
    assert_eq!(
        twap(&btc(), &small),
        Err(Refusal::UnderMinimum { size: dec("0.001"), price: dec("84000"), minimum: MINIMUM_TWAP_VALUE })
    );
    let unpriced = TwapIntent::new(Side::Buy, Amount::Notional(dec("500")), 30);
    assert_eq!(twap(&btc(), &unpriced), Err(Refusal::NoReference));
}
