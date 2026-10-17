use super::*;
use crate::trade::orders::trigger;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_stop_loss_at_market_bounds_itself_off_the_trigger() {
    let stop = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::StopLoss,
        Exit::market_within(dec("80000"), one_percent()),
    );
    assert_eq!(
        wire(trigger(&btc(), &stop, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"79200","s":"0.001","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"80000","tpsl":"sl"}}}],"grouping":"na"}"#
    );
}

#[test]
fn a_take_profit_can_rest_as_a_limit() {
    let take = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::TakeProfit,
        Exit::limit(dec("90000"), dec("89900")),
    );
    assert_eq!(
        wire(trigger(&btc(), &take, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"89900","s":"0.001","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"90000","tpsl":"tp"}}}],"grouping":"na"}"#
    );
}

/// The venue's TP/SL page: a market TP/SL takes 10% of slippage, which is
/// what [`Exit::market`] bounds it by.

#[test]
fn a_market_exit_takes_the_venue_s_ten_percent() {
    assert_eq!(TRIGGER_SLIPPAGE, dec("0.1"));
    let stop = TriggerIntent::new(Side::Sell, Covers::Position, Tpsl::StopLoss, Exit::market(dec("75000")));
    assert_eq!(
        wire(trigger(&btc(), &stop, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"67500","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"positionTpsl"}"#,
        "the whole position: size 0, grouped positionTpsl"
    );
}

#[test]
fn a_trigger_rounds_to_the_nearest_tick_and_spot_takes_none() {
    let stop = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::StopLoss,
        Exit::market_within(dec("80000.6"), one_percent()),
    );
    assert!(wire(trigger(&btc(), &stop, None)).contains(r#""triggerPx":"80001""#));
    assert_eq!(trigger(&hype(), &stop, None), Err(Refusal::Spot("a take profit or stop loss")));
}
