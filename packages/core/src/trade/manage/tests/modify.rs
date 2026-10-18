//! A resting order, as `frontendOpenOrders` lists it, replaced in place: on the grid as a new order
//! would be, and refused where the change does not fit the order.

use super::*;
use crate::trade::manage::modify;
use crate::trade::ticket_support::*;
use crate::trade::*;

fn at_market(trigger: &str, size: Covers) -> Change {
    Change::Trigger { trigger: dec(trigger), price: ExitPrice::Market { slippage: one_percent() }, size }
}

fn sized(size: &str) -> Covers {
    Covers::Size(Amount::Size(dec(size)))
}

#[test]
fn a_change_lands_on_the_grid_as_the_order_it_replaces() {
    let limit = resting(7, "B", "80000", "0.01", "Limit", None);
    let stop = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let take = resting(11, "A", "89900", "0.01", "Take Profit Limit", Some("90000"));
    let whole = resting(12, "A", "78705", "0.0", "Stop Market", Some("79500"));
    let moved_limit = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Gtc };
    let moved_take =
        Change::Trigger { trigger: dec("91000.57"), price: ExitPrice::Limit(dec("90950.77")), size: sized("0.01") };
    for (order, change, posted) in [
        (
            limit,
            moved_limit,
            r#"{"type":"batchModify","modifies":[{"oid":7,"order":{"a":0,"b":true,"p":"80500","s":"0.02","r":false,"t":{"limit":{"tif":"Gtc"}}}}]}"#,
        ),
        // At market, bounded one percent past the new trigger, as a new stop is; a trigger always places.
        (
            stop,
            at_market("78000", sized("0.01")),
            r#"{"type":"batchModify","modifies":[{"oid":9,"order":{"a":0,"b":false,"p":"77220","s":"0.01","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"78000","tpsl":"sl"}}}}],"a":true}"#,
        ),
        // The sell's limit rounds up, the trigger to the nearest tick.
        (
            take,
            moved_take,
            r#"{"type":"batchModify","modifies":[{"oid":11,"order":{"a":0,"b":false,"p":"90951","s":"0.01","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"91001","tpsl":"tp"}}}}],"a":true}"#,
        ),
        // A position's stop keeps size 0, and the minimum does not apply.
        (
            whole,
            at_market("78000", Covers::Position),
            r#"{"type":"batchModify","modifies":[{"oid":12,"order":{"a":0,"b":false,"p":"77220","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"78000","tpsl":"sl"}}}}],"a":true}"#,
        ),
    ] {
        assert_eq!(wire(modify(&btc(), &order, &change)), posted);
    }
}

#[test]
fn a_change_that_does_not_fit_the_order_is_refused() {
    let limit = resting(7, "B", "80000", "0.01", "Limit", None);
    let stop = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let whole = resting(12, "A", "78705", "0.0", "Stop Market", Some("79500"));
    let trailing = resting(10, "A", "78705", "0.01", "Trailing Stop", Some("79500"));
    let to_limit = |tif: Tif| Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif };
    let to_trigger =
        Change::Trigger { trigger: dec("78000"), price: ExitPrice::Limit(dec("78000")), size: Covers::Position };
    for (order, change, refused) in [
        (&limit, to_limit(Tif::Ioc), Refusal::ModifyTif(Tif::Ioc)),
        (&stop, to_limit(Tif::Gtc), Refusal::NotALimit(9)),
        (&limit, to_trigger, Refusal::NotATrigger(7)),
        (&stop, at_market("78000", Covers::Position), Refusal::NotWholePosition(9)),
        (&trailing, to_trigger, Refusal::UnknownTrigger { oid: 10, kind: "trailing stop".to_owned() }),
    ] {
        assert_eq!(modify(&btc(), order, &change), Err(refused));
    }
    assert!(
        matches!(modify(&btc(), &whole, &at_market("78000", sized("0.00001"))), Err(Refusal::UnderMinimum { .. })),
        "a size typed in is held to the minimum as any order is"
    );
    assert_eq!(
        modify(&btc(), &trailing, &to_trigger).unwrap_err().to_string(),
        "order 10 is a trailing stop trigger, which this build cannot modify"
    );
}
