use super::*;
use crate::trade::orders::order;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::sign::L1Action;

#[test]
fn an_entry_with_its_take_profit_and_stop_is_one_grouped_action() {
    let intent = OrderIntent::market(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), dec("0.005"))
        .with_cloid(Cloid::from_u128(0x7065_7270_0000_0000_0000_0000_0000_0001))
        .take_profit(Exit::market(dec("90000")))
        .stop_loss(Exit::market(dec("75000")));
    let placed = order(&btc(), &intent, None).unwrap();
    assert_eq!(placed.grouping(), crate::trade::orders::Grouping::NormalTpsl);
    assert_eq!(
        placed.wire(),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"80400","s":"0.00375","r":false,"t":{"limit":{"tif":"FrontendMarket"}},"c":"0x70657270000000000000000000000001"},{"a":0,"b":false,"p":"81000","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"90000","tpsl":"tp"}}},{"a":0,"b":false,"p":"67500","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"normalTpsl"}"#
    );
    let alone = OrderIntent::market(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), dec("0.005"));
    assert_eq!(
        order(&btc(), &alone, None).unwrap().grouping(),
        crate::trade::orders::Grouping::Na,
        "no exit, no group"
    );
}

#[test]
fn an_exit_on_the_wrong_side_of_the_entry_is_refused() {
    let long = limit(Side::Buy, "80000", "0.001", Tif::Gtc);
    assert_eq!(
        order(&btc(), &long.take_profit(Exit::market(dec("79000"))), None),
        Err(Refusal::ExitSide { tpsl: Tpsl::TakeProfit, trigger: dec("79000"), entry: dec("80000") })
    );
    assert_eq!(
        order(&btc(), &long.stop_loss(Exit::market(dec("81000"))), None),
        Err(Refusal::ExitSide { tpsl: Tpsl::StopLoss, trigger: dec("81000"), entry: dec("80000") })
    );
    let short = limit(Side::Sell, "80000", "0.001", Tif::Gtc).take_profit(Exit::market(dec("79000")));
    assert!(order(&btc(), &short, None).is_ok(), "a short's take profit is below");
}
