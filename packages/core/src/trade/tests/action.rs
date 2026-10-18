//! Every trading action as one value: what is shown is what is signed.

use crate::utils::sign::L1Action;
use crate::utils::{Asset, Dec};

use super::*;
use crate::trade::manage::{BatchModify, Cancel, CancelByCloid, Modify, Noop, OrderRef, ScheduleCancel};
use crate::trade::margin::{MarginMode, UpdateIsolatedMargin, UpdateLeverage};
use crate::trade::orders::{Cloid, Order, PlaceOrder, Side, Tif, Tpsl, Twap, TwapCancel};

fn limit() -> Order {
    Order::limit(Asset::from_id(0), Side::Buy, Dec::from_int(84_000), Dec::ONE, Tif::Gtc)
}

/// Each action's `type`, and how many statuses the venue answers it with: one an order, cancel or
/// modify of a batch, and none counted for the rest.
#[test]
fn each_action_keeps_its_own_wire_type_and_count_of_statuses() {
    let twice = vec![(Asset::from_id(0), 1), (Asset::from_id(0), 2)];
    let cases: [(TradeAction, &str, Option<usize>); 11] = [
        (PlaceOrder::new(vec![limit(), limit()]).into(), "order", Some(2)),
        (Cancel::new(twice).into(), "cancel", Some(2)),
        (CancelByCloid::one(Asset::from_id(0), Cloid::from_u128(7)).into(), "cancelByCloid", Some(1)),
        (Modify::new(OrderRef::Oid(7), limit()).into(), "modify", None),
        (BatchModify::new(vec![Modify::new(OrderRef::Oid(7), limit())]).into(), "batchModify", Some(1)),
        (Twap::new(Asset::from_id(0), Side::Buy, Dec::ONE, 30).unwrap().into(), "twapOrder", None),
        (TwapCancel::new(Asset::from_id(0), 7).into(), "twapCancel", None),
        (UpdateLeverage::new(Asset::from_id(0), MarginMode::Cross, 3).unwrap().into(), "updateLeverage", None),
        (
            UpdateIsolatedMargin::new(Asset::from_id(0), Side::Buy, Dec::ONE).unwrap().into(),
            "updateIsolatedMargin",
            None,
        ),
        (ScheduleCancel::disarm().into(), "scheduleCancel", None),
        (Noop.into(), "noop", None),
    ];
    for (action, kind, items) in cases {
        assert_eq!((action.kind(), action.items()), (kind, items));
        assert!(action.wire().starts_with(&format!(r#"{{"type":"{kind}""#)), "{}", action.wire());
    }
    assert_eq!(TradeAction::from(Noop).wire(), Noop.wire(), "what is shown is what is signed");
}

/// The exchange endpoint docs: "Both single and batch modify actions include an optional
/// `always_place` flag" that a trigger needs, written only when set, and last.
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
