//! A take profit or stop loss on its own: it fires at the nearest tick, then fills at market within
//! its bound or as a limit, reduce only; for a fixed size grouped `na`, for the whole position at
//! size 0 grouped `positionTpsl`.

use super::*;
use crate::trade::orders::trigger;
use crate::trade::ticket_support::*;
use crate::trade::*;

/// The venue's TP/SL page: a market TP/SL takes 10% of slippage, which is what [`Exit::market`]
/// bounds it by.
#[test]
fn a_trigger_fires_at_the_nearest_tick_and_fills_within_its_bound_or_at_its_limit() {
    assert_eq!(TRIGGER_SLIPPAGE, dec("0.1"));
    let sized = Covers::Size(Amount::Size(dec("0.001")));
    let sell = |covers: Covers, tpsl: Tpsl, exit: Exit| TriggerIntent::new(Side::Sell, covers, tpsl, exit);
    for (intent, posted) in [
        (
            sell(sized, Tpsl::StopLoss, Exit::market_within(dec("80000"), one_percent())),
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"79200","s":"0.001","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"80000","tpsl":"sl"}}}],"grouping":"na"}"#,
        ),
        (
            sell(sized, Tpsl::TakeProfit, Exit::limit(dec("90000"), dec("89900"))),
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"89900","s":"0.001","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"90000","tpsl":"tp"}}}],"grouping":"na"}"#,
        ),
        (
            sell(Covers::Position, Tpsl::StopLoss, Exit::market(dec("75000"))),
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"67500","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"positionTpsl"}"#,
        ),
        (
            sell(sized, Tpsl::StopLoss, Exit::market_within(dec("80000.6"), one_percent())),
            r#"{"type":"order","orders":[{"a":0,"b":false,"p":"79201","s":"0.001","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"80001","tpsl":"sl"}}}],"grouping":"na"}"#,
        ),
    ] {
        assert_eq!(wire(trigger(&btc(), &intent, None)), posted);
    }
    let stop = sell(sized, Tpsl::StopLoss, Exit::market(dec("80000")));
    assert_eq!(trigger(&hype(), &stop, None), Err(Refusal::Spot("a take profit or stop loss")));
    assert!(matches!(
        trigger(
            &btc(),
            &sell(Covers::Size(Amount::Size(dec("0.0001"))), Tpsl::StopLoss, Exit::market(dec("80000"))),
            None
        ),
        Err(Refusal::UnderMinimum { .. })
    ));
}
