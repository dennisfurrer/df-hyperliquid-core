use crate::live::fixtures::{SECOND, address, dec, mainnet, read};
use crate::live::{Frame, decode};
use crate::markets::Side;
use crate::trade::Cloid;

#[test]
fn order_updates_and_historical_orders_carry_status_and_order() {
    let Frame::OrderUpdates(updates) = read(&mainnet("orderUpdates")) else { panic!("updates") };
    assert_eq!(updates.len(), 1);
    let update = &updates[0];
    assert_eq!((update.status.as_str(), update.status_timestamp), ("open", 1_791_480_448_890));
    let order = &update.order;
    assert_eq!(
        (order.coin.as_str(), order.side, order.limit_px, order.sz),
        ("BTC", Side::Buy, dec("80462.0"), dec("0.20705"))
    );
    assert_eq!((order.oid, order.timestamp, order.orig_sz), (569_017_209_592, 1_791_480_448_890, dec("0.20705")));
    assert_eq!(order.cloid.map(Cloid::to_hex).as_deref(), Some("0x273d565c96b2523ae6a0b5ca299c6f00"));

    let Frame::HistoricalOrders(history) = read(&mainnet("userHistoricalOrders")) else { panic!("history") };
    assert_eq!((history.user, history.is_snapshot, history.orders.len()), (address(SECOND), false, 1));
    let past = &history.orders[0];
    assert_eq!((past.status.word(), past.order.oid, past.order.tif.as_deref()), ("open", 569_017_209_592, Some("Alo")));
}

/// Every recorded resting order, sent or read, says its type, its trigger and whether it is a
/// position's TP/SL: one without them is refused, not read as a plain limit.
#[test]
fn a_resting_order_without_its_type_or_trigger_is_refused() {
    let sent = mainnet("openOrders");
    assert!(decode(&sent).is_ok());
    for gone in [r#""orderType":"Limit","#, r#""triggerCondition":"N/A","#, r#""isPositionTpsl":false,"#] {
        assert!(decode(&sent.replace(gone, "")).is_err(), "without {gone}");
    }
}
