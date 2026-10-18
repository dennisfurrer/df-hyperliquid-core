//! The order rows as the venue writes them. Live answers, mainnet, 2026-10-08, trimmed to a few whole
//! rows each: one account's resting orders, and another's order history, one row of each status,
//! order type and time in force. The reads, and the statuses by either id, run in the mocked
//! journeys.

use crate::utils::Dec;

use super::*;
use crate::trade::orders::{Side, Tif, Tpsl};

const OPEN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/open_orders.json"));
const FRONTEND: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/frontend_open_orders.json"));
const HISTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/historical_orders.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// Every recorded resting order, read or sent, carries its original size: a row without it is
/// refused, not read as a size unknown.
#[test]
fn an_open_order_reads_reduce_only_where_written_and_needs_its_original_size() {
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
    let mut row: serde_json::Value = serde_json::from_str::<Vec<serde_json::Value>>(OPEN).unwrap().remove(1);
    row.as_object_mut().unwrap().remove("origSz");
    assert!(serde_json::from_value::<OpenOrder>(row).is_err());
}

/// A kind is read from the venue's words, and a word this build does not know is kept as written.
#[test]
fn a_frontend_order_says_what_kind_of_order_it_is_in_the_venue_s_words() {
    for (word, kind, tpsl, at_market) in [
        ("Limit", OrderKind::Limit, None, false),
        ("Market", OrderKind::Market, None, true),
        ("Stop Market", OrderKind::StopMarket, Some(Tpsl::StopLoss), true),
        ("Stop Limit", OrderKind::StopLimit, Some(Tpsl::StopLoss), false),
        ("Take Profit Market", OrderKind::TakeProfitMarket, Some(Tpsl::TakeProfit), true),
        ("Take Profit Limit", OrderKind::TakeProfitLimit, Some(Tpsl::TakeProfit), false),
        ("Liquidation Market", OrderKind::Other("Liquidation Market".to_owned()), None, false),
        ("stop market", OrderKind::Other("stop market".to_owned()), None, false),
    ] {
        assert_eq!(OrderKind::parse(word), kind, "{word}");
        assert_eq!((kind.word(), kind.tpsl(), kind.fills_at_market()), (word, tpsl, at_market), "{word}");
    }
    let orders: Vec<FrontendOrder> = serde_json::from_str(FRONTEND).unwrap();
    let stop = &orders[0];
    assert_eq!(
        (stop.order_type.clone(), stop.trigger(), stop.tpsl()),
        (OrderKind::StopMarket, Some(dec("6.04")), Some(Tpsl::StopLoss))
    );
    assert_eq!(
        (stop.trigger_condition.as_str(), stop.tif.as_deref(), stop.time_in_force(), stop.is_position_tpsl),
        ("Price above 6.04", None, None, true)
    );
    let limit = &orders[1];
    assert_eq!((limit.order_type.clone(), limit.trigger(), limit.tpsl()), (OrderKind::Limit, None, None));
    assert_eq!((limit.time_in_force(), limit.orig_sz), (Some(Tif::Gtc), dec("0.00126")));
    assert!(limit.children.is_empty());
    let take = &orders[2];
    assert_eq!((take.side, take.trigger()), (Side::Sell, Some(dec("0.009572"))));
}

#[test]
fn every_status_reads_as_its_kind_of_end_and_an_unread_lookup_is_refused() {
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
    for (word, state) in [
        ("scheduledCancel", OrderState::Canceled("scheduledCancel".to_owned())),
        ("rejected", OrderState::Rejected("rejected".to_owned())),
        ("new", OrderState::Other("new".to_owned())),
    ] {
        assert_eq!(serde_json::from_str::<OrderState>(&format!("\"{word}\"")).unwrap(), state, "{word}");
    }
    let unread = serde_json::from_str::<OrderLookup>(r#"{"status":"somethingElse"}"#).unwrap_err();
    assert!(unread.to_string().contains("orderStatus answered `somethingElse`"), "{unread}");
}
