//! An account's orders on the socket: each status change, the order history, and the orders
//! resting on one dex with their type and trigger.

use crate::live::Frame;
use crate::live::fixtures::{SECOND, address, dec, mainnet, read, vault};
use crate::markets::Side;
use crate::trade::Cloid;
use crate::utils::Dec;

#[test]
fn order_updates_history_and_resting_orders_carry_each_order_whole() {
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
    let Frame::HistoricalOrders(opened) = read(&vault("userHistoricalOrders")) else { panic!("history") };
    assert!(opened.is_snapshot && opened.orders.is_empty(), "the history opens with a snapshot");

    let Frame::OpenOrders(open) = read(&mainnet("openOrders")) else { panic!("orders") };
    assert_eq!((open.dex.as_str(), open.user, open.orders.len()), ("", address(SECOND), 1));
    let order = &open.orders[0];
    assert_eq!(
        (order.limit_px, order.sz, order.orig_sz, order.trigger_px),
        (dec("80459.0"), dec("0.20706"), dec("0.20706"), Dec::ZERO)
    );
    assert_eq!(
        (order.order_type.word(), order.tif.as_deref(), order.trigger_condition.as_str()),
        ("Limit", Some("Alo"), "N/A")
    );
    assert!(!order.is_trigger && !order.is_position_tpsl && !order.reduce_only && order.children.is_empty());
    let Frame::OpenOrders(none) = read(&mainnet("openOrders.empty")) else { panic!("orders") };
    assert!(none.orders.is_empty());
}
