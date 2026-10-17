use super::*;
use crate::trade::manage::modify;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_modify_replaces_the_chosen_order_in_place() {
    let order = resting(7, "B", "80000", "0.01", "Limit", None);
    let change = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Gtc };
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":7,"order":{"a":0,"b":true,"p":"80500","s":"0.02","r":false,"t":{"limit":{"tif":"Gtc"}}}}]}"#
    );
    let ioc = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Ioc };
    assert_eq!(modify(&btc(), &order, &ioc), Err(Refusal::ModifyTif(Tif::Ioc)), "without always place: Alo or Gtc");
}

#[test]
fn a_stop_is_modified_in_place_as_a_trigger_that_always_places() {
    let order = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let change = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Size(Amount::Size(dec("0.01"))),
    };
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":9,"order":{"a":0,"b":false,"p":"77220","s":"0.01","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"78000","tpsl":"sl"}}}}],"a":true}"#,
        "at market, bounded one percent past the new trigger, as a new stop is"
    );
}

#[test]
fn a_take_profit_modified_as_a_limit_is_brought_onto_the_grid() {
    let order = resting(11, "A", "89900", "0.01", "Take Profit Limit", Some("90000"));
    let change = Change::Trigger {
        trigger: dec("91000.57"),
        price: ExitPrice::Limit(dec("90950.77")),
        size: Covers::Size(Amount::Size(dec("0.01"))),
    };
    // the sell's limit rounds up, the trigger to the nearest tick
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":11,"order":{"a":0,"b":false,"p":"90951","s":"0.01","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"91001","tpsl":"tp"}}}}],"a":true}"#
    );
}

#[test]
fn a_position_s_stop_keeps_size_zero_and_the_minimum_does_not_apply() {
    let order = resting(12, "A", "78705", "0.0", "Stop Market", Some("79500"));
    let whole = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Position,
    };
    assert!(wire(modify(&btc(), &order, &whole)).contains(r#""s":"0","r":true"#));
    let tiny = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Size(Amount::Size(dec("0.00001"))),
    };
    assert!(
        matches!(modify(&btc(), &order, &tiny), Err(Refusal::UnderMinimum { .. })),
        "a size typed in is held to the minimum as any order is"
    );
    let sized = resting(13, "A", "78705", "0.01", "Stop Market", Some("79500"));
    assert_eq!(modify(&btc(), &sized, &whole), Err(Refusal::NotWholePosition(13)));
}

#[test]
fn a_change_that_does_not_fit_the_order_is_refused() {
    let stop = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let limit_change = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Gtc };
    assert_eq!(modify(&btc(), &stop, &limit_change), Err(Refusal::NotALimit(9)));
    let plain = resting(7, "B", "80000", "0.01", "Limit", None);
    let trigger_change =
        Change::Trigger { trigger: dec("78000"), price: ExitPrice::Limit(dec("78000")), size: Covers::Position };
    assert_eq!(modify(&btc(), &plain, &trigger_change), Err(Refusal::NotATrigger(7)));
    let trailing = resting(10, "A", "78705", "0.01", "Trailing Stop", Some("79500"));
    assert_eq!(
        modify(&btc(), &trailing, &trigger_change).unwrap_err().to_string(),
        "order 10 is a trailing stop trigger, which this build cannot modify"
    );
}
