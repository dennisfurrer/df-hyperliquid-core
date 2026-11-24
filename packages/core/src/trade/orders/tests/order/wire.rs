//! What an order posts that no vector pins: a market order's tif, and the key order and spellings the venue
//! hashes.

use crate::trade::manage::CancelByCloid;
use crate::trade::manage::{BatchModify, Modify, OrderRef};
use crate::trade::orders::*;
use crate::utils::rounding::Grid;
use crate::utils::sign::L1Action;

use super::{BUILDER, asset, dec, limit, through};

/// The rule the whole family keeps: a market order is `FrontendMarket`, on
/// either side, at a bound given or one the grid makes, and nothing a market
/// order posts says `Gtc`.
#[test]
fn a_market_order_is_frontend_market_and_never_gtc() {
    for side in [Side::Buy, Side::Sell] {
        let order = Order::market(asset(0), side, dec("86501"), dec("0.0012"));
        assert_eq!(order.tif(), Some(Tif::FrontendMarket), "{side:?}");
        assert!(order.is_market());
        let posted = PlaceOrder::one(order).wire();
        assert!(posted.contains(r#""tif":"FrontendMarket""#), "{posted}");
        assert!(!posted.contains("Gtc"), "{posted}");

        let grid = Grid::perp(5);
        let bound = grid.market_bound(dec("86500"), dec("0.01"), side).unwrap();
        let gridded = Order::market(asset(0), side, bound, grid.size(dec("0.0012")).unwrap());
        assert_eq!(gridded.tif(), Some(Tif::FrontendMarket), "{side:?}");
        assert!(gridded.is_market());
    }
    let resting = Order::limit(asset(0), Side::Buy, dec("86501"), dec("0.0012"), Tif::Gtc);
    assert!(!resting.is_market(), "a Gtc is a resting limit, not a market order");
    let stop = Order::trigger(asset(0), Side::Sell, dec("1"), dec("1"), dec("2"), Tpsl::StopLoss, true);
    assert_eq!((stop.tif(), stop.is_market(), stop.is_trigger()), (None, false, true));
}

/// No SDK vector covers a cloid, `cancelByCloid` or a single `modify`, so their spellings and key
/// order are pinned here: `a b p s r t c` for an order, `grouping`, then `builder` last.
#[test]
fn keys_go_in_the_order_the_venue_hashes_and_ids_as_it_spells_them() {
    let order = limit(516, Side::Buy, "0.35", "50", Tif::Gtc);
    let cloid: Cloid = "0x00000000000000000000000000000009".parse().unwrap();
    assert_eq!((cloid, cloid.to_string()), (Cloid::from_u128(9), "0x00000000000000000000000000000009".to_owned()));
    assert!("0xabcd".parse::<Cloid>().is_err(), "a cloid is thirty-two hex digits");
    assert_eq!(order.wire().keys().collect::<Vec<_>>(), ["a", "b", "p", "s", "r", "t"]);
    assert_eq!(order.wire().to_msgpack()[0], 0x86, "an absent cloid leaves no key, not a nil");
    assert_eq!(order.with_cloid(cloid).wire().keys().collect::<Vec<_>>(), ["a", "b", "p", "s", "r", "t", "c"]);
    assert_eq!(PlaceOrder::one(order).action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping"]);
    let routed = through(order, BUILDER, 10);
    assert_eq!(routed.action().keys().collect::<Vec<_>>(), ["type", "orders", "grouping", "builder"]);

    assert_eq!(
        CancelByCloid::one(asset(100_005_160), cloid).wire(),
        r#"{"type":"cancelByCloid","cancels":[{"asset":100005160,"cloid":"0x00000000000000000000000000000009"}]}"#
    );
    let modify = Modify::new(OrderRef::Oid(42), limit(516, Side::Sell, "0.66", "1", Tif::Alo));
    assert_eq!(modify.action().keys().collect::<Vec<_>>(), ["type", "oid", "order"]);
    assert!(modify.wire().starts_with(r#"{"type":"modify","oid":42,"order":{"a":516,"#));
    let by_client = Modify::new(OrderRef::Cloid(cloid), limit(516, Side::Sell, "0.66", "1", Tif::Alo));
    assert!(by_client.wire().contains(r#""oid":"0x00000000000000000000000000000009""#));
    assert!(!BatchModify::new(vec![modify]).is_always_place(), "a limit order needs no always place");
    assert!(BatchModify::new(vec![modify]).always_place().wire().ends_with(r#""a":true}"#));
}

#[test]
fn an_order_built_from_its_own_type_is_the_same_order() {
    let btc = asset(0);
    let limit = Order::limit(btc, Side::Buy, Dec::parse("100").unwrap(), Dec::parse("1").unwrap(), Tif::Alo);
    let again = Order::new(limit.asset(), limit.side(), limit.price(), limit.size(), limit.order_type());
    assert_eq!(again, limit);
    let trigger = Order::trigger(
        btc,
        Side::Sell,
        Dec::parse("90").unwrap(),
        Dec::ZERO,
        Dec::parse("95").unwrap(),
        Tpsl::StopLoss,
        true,
    );
    let again = Order::new(trigger.asset(), trigger.side(), trigger.price(), trigger.size(), trigger.order_type());
    assert_eq!(again, trigger);
}
