//! An entry's take profit and stop loss: grouped `normalTpsl` with it, reduce only the other way,
//! and each on its own side of the entry.

use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;

#[test]
fn an_entry_with_its_take_profit_and_stop_is_one_grouped_action() {
    let alone = OrderIntent::market(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), dec("0.005"));
    let bracketed = alone
        .with_cloid(Cloid::from_u128(0x7065_7270_0000_0000_0000_0000_0000_0001))
        .take_profit(Exit::market(dec("90000")))
        .stop_loss(Exit::market(dec("75000")));
    let placed = order(&btc(), &bracketed, None).unwrap();
    assert_eq!(placed.grouping(), Grouping::NormalTpsl);
    assert_eq!(
        placed.wire(),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"80400","s":"0.00375","r":false,"t":{"limit":{"tif":"FrontendMarket"}},"c":"0x70657270000000000000000000000001"},{"a":0,"b":false,"p":"81000","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"90000","tpsl":"tp"}}},{"a":0,"b":false,"p":"67500","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"normalTpsl"}"#
    );
    assert_eq!(order(&btc(), &alone, None).unwrap().grouping(), Grouping::Na, "no exit, no group");
}

/// Above the entry for a long's take profit and a short's stop, below it otherwise.
#[test]
fn an_exit_on_the_wrong_side_of_the_entry_is_refused() {
    let long = limit(Side::Buy, "80000", "0.001", Tif::Gtc);
    let short = limit(Side::Sell, "80000", "0.001", Tif::Gtc);
    let refused =
        |tpsl: Tpsl, trigger: &str| Err(Refusal::ExitSide { tpsl, trigger: dec(trigger), entry: dec("80000") });
    for (intent, expected) in [
        (long.take_profit(Exit::market(dec("79000"))), refused(Tpsl::TakeProfit, "79000")),
        (long.stop_loss(Exit::market(dec("81000"))), refused(Tpsl::StopLoss, "81000")),
        (short.take_profit(Exit::market(dec("81000"))), refused(Tpsl::TakeProfit, "81000")),
        (short.stop_loss(Exit::market(dec("79000"))), refused(Tpsl::StopLoss, "79000")),
    ] {
        assert_eq!(order(&btc(), &intent, None).map(|placed| placed.orders().len()), expected, "{intent:?}");
    }
    assert!(
        order(&btc(), &short.take_profit(Exit::market(dec("79000"))), None).is_ok(),
        "a short's take profit is below"
    );
    assert_eq!(
        order(&btc(), &long.stop_loss(Exit::market(dec("81000"))), None).unwrap_err().to_string(),
        "a stop loss at 81000 is on the wrong side of an entry at 80000"
    );
}
