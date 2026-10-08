//! Every trading action as one value: what is shown is what is signed.

use hl_core::{Asset, Dec, L1Action};

use super::*;
use crate::{
    cancel::Cancel,
    modify::{BatchModify, Modify, OrderRef},
    order::{Order, PlaceOrder, Side, Tif, Tpsl},
    schedule::{Noop, ScheduleCancel},
};

fn limit() -> Order {
    Order::limit(Asset::from_id(0), Side::Buy, Dec::from_int(84_000), Dec::ONE, Tif::Gtc)
}

#[test]
fn each_action_keeps_its_own_wire_and_type() {
    let actions: Vec<TradeAction> = vec![
        PlaceOrder::one(limit()).into(),
        Cancel::one(Asset::from_id(0), 7).into(),
        BatchModify::new(vec![Modify::new(OrderRef::Oid(7), limit())]).into(),
        ScheduleCancel::disarm().into(),
        Noop.into(),
    ];
    let kinds: Vec<&str> = actions.iter().map(TradeAction::kind).collect();
    assert_eq!(kinds, ["order", "cancel", "batchModify", "scheduleCancel", "noop"]);
    for action in &actions {
        assert!(action.wire().starts_with(&format!(r#"{{"type":"{}""#, action.kind())), "{}", action.wire());
    }
    assert_eq!(TradeAction::from(Noop).wire(), Noop.wire());
}

#[test]
fn the_statuses_an_action_answers_with_are_counted() {
    let two = PlaceOrder::new(vec![limit(), limit()]);
    assert_eq!(TradeAction::from(two).items(), Some(2));
    assert_eq!(TradeAction::from(Cancel::new(vec![(Asset::from_id(0), 1), (Asset::from_id(0), 2)])).items(), Some(2));
    assert_eq!(TradeAction::from(Noop).items(), None, "answers default");
}

/// The exchange endpoint page: "Both single and batch modify actions include
/// an optional `always_place` flag" that a trigger needs, written only when
/// set, last, where the SDK's `modify` schema puts it.
#[test]
fn a_single_modify_of_a_trigger_always_places() {
    let stop = Order::trigger(
        Asset::from_id(0),
        Side::Sell,
        Dec::from_int(79_200),
        Dec::ONE,
        Dec::from_int(80_000),
        Tpsl::StopLoss,
        true,
    )
    .reduce_only();
    let modify = Modify::new(OrderRef::Oid(9), stop);
    assert!(modify.is_always_place());
    assert_eq!(modify.action().keys().collect::<Vec<_>>(), ["type", "oid", "order", "a"]);
    assert!(modify.wire().ends_with(r#","a":true}"#), "{}", modify.wire());
    assert_eq!(modify.entry().keys().collect::<Vec<_>>(), ["oid", "order"], "a batch's entry carries none");
    let plain = Modify::new(OrderRef::Oid(9), limit());
    assert!(!plain.is_always_place());
    assert_eq!(plain.action().keys().collect::<Vec<_>>(), ["type", "oid", "order"]);
    assert!(plain.always_place().wire().ends_with(r#","a":true}"#));
}
