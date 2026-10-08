//! Live answers, mainnet, 2026-10-08, trimmed to a few rows each (every kept
//! row whole, in the venue's key order): one account's resting orders (a
//! position stop, a spot limit, a position take profit), the order status of
//! three of its orders and of one it never placed, and another account's
//! order history, one row of each status, order type and time in force.

use hl_core::{Address, Dec};

use super::*;
use crate::{
    modify::OrderRef,
    order::{Cloid, Side, Tif, Tpsl},
    tests_support::{Recorded, block_on},
};

const OPEN: &str = include_str!("../../fixtures/open_orders.json");
const FRONTEND: &str = include_str!("../../fixtures/frontend_open_orders.json");
const STATUS_OPEN: &str = include_str!("../../fixtures/order_status_open.json");
const STATUS_FILLED: &str = include_str!("../../fixtures/order_status_filled.json");
const STATUS_BY_CLOID: &str = include_str!("../../fixtures/order_status_by_cloid.json");
const STATUS_UNKNOWN: &str = include_str!("../../fixtures/order_status_unknown.json");
const HISTORY: &str = include_str!("../../fixtures/historical_orders.json");

const WATCHED: &str = "0xf77a75616ff4aa16c890fabb4ab725a732d7e2b7";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn user() -> Address {
    WATCHED.parse().unwrap()
}

#[test]
fn every_read_asks_in_the_venue_s_words() {
    assert_eq!(open_orders_request(user(), None), format!(r#"{{"type":"openOrders","user":"{WATCHED}"}}"#));
    assert_eq!(
        open_orders_request(user(), Some("xyz")),
        format!(r#"{{"type":"openOrders","user":"{WATCHED}","dex":"xyz"}}"#)
    );
    assert_eq!(
        frontend_open_orders_request(user(), Some("")),
        format!(r#"{{"type":"frontendOpenOrders","user":"{WATCHED}","dex":""}}"#)
    );
    assert_eq!(
        order_status_request(user(), OrderRef::Oid(569_021_780_586)),
        format!(r#"{{"type":"orderStatus","user":"{WATCHED}","oid":569021780586}}"#)
    );
    assert_eq!(
        order_status_request(user(), OrderRef::Cloid(Cloid::from_u128(0xba5e_d110_67f2_cc01_ba5e_d100_00ba_5ed1))),
        format!(r#"{{"type":"orderStatus","user":"{WATCHED}","oid":"0xba5ed11067f2cc01ba5ed10000ba5ed1"}}"#)
    );
    assert_eq!(historical_orders_request(user()), format!(r#"{{"type":"historicalOrders","user":"{WATCHED}"}}"#));
}

#[test]
fn open_orders_read_with_reduce_only_only_where_the_venue_writes_it() {
    let orders: Vec<OpenOrder> = serde_json::from_str(OPEN).unwrap();
    assert_eq!(orders.len(), 3);
    let stop = &orders[0];
    assert_eq!((stop.coin.as_str(), stop.side, stop.oid), ("ENS", Side::Buy, 569_021_780_586));
    assert_eq!((stop.limit_px, stop.sz, stop.orig_sz), (dec("6.5232"), Dec::ZERO, Dec::ZERO));
    assert!(stop.reduce_only);
    assert_eq!(stop.cloid, Some("0xba5ed11067f2cc01ba5ed10000ba5ed1".parse().unwrap()));
    let spot = &orders[1];
    assert_eq!((spot.coin.as_str(), spot.limit_px, spot.sz), ("@142", dec("79335"), dec("0.00126")));
    assert!(!spot.reduce_only, "absent is false");
    assert_eq!(orders[2].side, Side::Sell, "A is a sell");
}

/// Every recorded resting order, read or sent, carries its original size: a row without it is
/// refused, not read as a size unknown.
#[test]
fn an_open_order_without_its_original_size_is_refused() {
    let orders: Vec<serde_json::Value> = serde_json::from_str(OPEN).unwrap();
    let mut row = orders[1].clone();
    assert!(serde_json::from_value::<OpenOrder>(row.clone()).is_ok());
    row.as_object_mut().unwrap().remove("origSz");
    assert!(serde_json::from_value::<OpenOrder>(row).is_err());
}

/// A kind is read from the venue's words without going through serde, and a word this build
/// does not know is kept as written.
#[test]
fn an_order_kind_is_parsed_from_the_venue_s_words() {
    for kind in [
        OrderKind::Limit,
        OrderKind::Market,
        OrderKind::StopMarket,
        OrderKind::StopLimit,
        OrderKind::TakeProfitMarket,
        OrderKind::TakeProfitLimit,
    ] {
        assert_eq!(OrderKind::parse(kind.word()), kind);
    }
    assert_eq!(OrderKind::parse("Liquidation Market"), OrderKind::Other("Liquidation Market".to_owned()));
    assert_eq!(
        OrderKind::parse("stop market"),
        OrderKind::Other("stop market".to_owned()),
        "the venue's spelling only"
    );
}

#[test]
fn a_frontend_order_says_what_kind_of_order_it_is() {
    let orders: Vec<FrontendOrder> = serde_json::from_str(FRONTEND).unwrap();
    let stop = &orders[0];
    assert_eq!(stop.order_type, OrderKind::StopMarket);
    assert_eq!((stop.trigger(), stop.tpsl(), stop.is_position_tpsl), (Some(dec("6.04")), Some(Tpsl::StopLoss), true));
    assert_eq!(
        (stop.trigger_condition.as_str(), stop.tif.as_deref(), stop.time_in_force()),
        ("Price above 6.04", None, None)
    );
    assert!(stop.order_type.fills_at_market());
    let limit = &orders[1];
    assert_eq!((limit.order_type.clone(), limit.trigger(), limit.tpsl()), (OrderKind::Limit, None, None));
    assert_eq!((limit.time_in_force(), limit.orig_sz), (Some(Tif::Gtc), dec("0.00126")));
    assert!(limit.children.is_empty());
    let take = &orders[2];
    assert_eq!(
        (take.order_type.clone(), take.tpsl(), take.side),
        (OrderKind::TakeProfitMarket, Some(Tpsl::TakeProfit), Side::Sell)
    );
    assert_eq!(take.trigger(), Some(dec("0.009572")));
}

#[test]
fn an_order_s_status_is_found_or_unknown() {
    let OrderLookup::Found(open) = serde_json::from_str(STATUS_OPEN).unwrap() else { panic!("found") };
    assert_eq!(
        (open.status.clone(), open.order.oid, open.status_timestamp),
        (OrderState::Open, 569_021_780_586, 1_791_480_626_296)
    );
    let OrderLookup::Found(filled) = serde_json::from_str(STATUS_FILLED).unwrap() else { panic!("found") };
    assert_eq!(filled.status, OrderState::Filled);
    assert_eq!(filled.order.time_in_force(), Some(Tif::FrontendMarket));
    assert_eq!(
        (filled.order.order_type.clone(), filled.order.orig_sz, filled.order.sz),
        (OrderKind::Market, dec("4095"), Dec::ZERO)
    );
    let OrderLookup::Found(by_cloid) = serde_json::from_str(STATUS_BY_CLOID).unwrap() else { panic!("found") };
    assert_eq!(by_cloid.order.trigger_condition, "Triggered", "a stop that fired reads as no trigger any more");
    assert_eq!(by_cloid.order.trigger(), None);
    assert_eq!(serde_json::from_str::<OrderLookup>(STATUS_UNKNOWN).unwrap(), OrderLookup::Unknown);
}

#[test]
fn every_status_in_the_history_reads_as_its_kind_of_end() {
    let history: Vec<OrderRecord> = serde_json::from_str(HISTORY).unwrap();
    let states: Vec<OrderState> = history.iter().map(|record| record.status.clone()).collect();
    for (state, word) in [
        (OrderState::Filled, "filled"),
        (OrderState::Open, "open"),
        (OrderState::Triggered, "triggered"),
        (OrderState::Canceled("canceled".to_owned()), "canceled"),
        (OrderState::Canceled("reduceOnlyCanceled".to_owned()), "reduceOnlyCanceled"),
        (OrderState::Canceled("siblingFilledCanceled".to_owned()), "siblingFilledCanceled"),
        (OrderState::Rejected("reduceOnlyRejected".to_owned()), "reduceOnlyRejected"),
        (OrderState::Rejected("minTradeNtlRejected".to_owned()), "minTradeNtlRejected"),
    ] {
        assert!(states.contains(&state), "{word}");
        assert_eq!(state.word(), word);
    }
    let liquidation = history.iter().find(|record| record.order.tif.as_deref() == Some("LiquidationMarket")).unwrap();
    assert_eq!(liquidation.order.time_in_force(), None, "a TIF only the venue sends is kept, not guessed");
    assert_eq!(
        serde_json::from_str::<OrderState>(r#""scheduledCancel""#).unwrap(),
        OrderState::Canceled("scheduledCancel".to_owned()),
        "the dead man's switch is a cancel"
    );
    assert_eq!(
        serde_json::from_str::<OrderState>(r#""rejected""#).unwrap(),
        OrderState::Rejected("rejected".to_owned())
    );
    assert_eq!(serde_json::from_str::<OrderState>(r#""new""#).unwrap(), OrderState::Other("new".to_owned()));
}

#[test]
fn each_read_goes_through_the_transport_once() {
    let transport = Recorded::answering(FRONTEND);
    let orders = block_on(frontend_open_orders(&transport, user(), None)).unwrap();
    assert_eq!(orders.len(), 3);
    let transport = Recorded::answering(OPEN);
    assert_eq!(block_on(open_orders(&transport, user(), None)).unwrap().len(), 3);
    let transport = Recorded::answering(STATUS_UNKNOWN);
    assert_eq!(block_on(order_status(&transport, user(), OrderRef::Oid(1))).unwrap(), OrderLookup::Unknown);
    assert_eq!(transport.sent(), [("/info".to_owned(), order_status_request(user(), OrderRef::Oid(1)))]);
    let transport = Recorded::answering(HISTORY);
    assert_eq!(block_on(historical_orders(&transport, user())).unwrap().len(), 16);
}
