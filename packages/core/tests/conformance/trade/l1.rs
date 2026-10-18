//! `l1.json`, by `@nktkas/hyperliquid` 0.24.3: each L1 case built by a typed builder and signed with the
//! file's key.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::trade::{
    Amount, Cancel, Cloid, Covers, Exit, MarginMode, Market, Order, OrderIntent, PlaceOrder, ScheduleCancel, Side, Tif,
    Tpsl, TriggerIntent, margin, orders,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Asset, Json, OutcomeSide};

use super::{btc, check, cloid, dec, exit, flat, id, limit, parse, text};

const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));

fn l1_0_24_3(name: &str, now: u64) -> Box<dyn L1Action> {
    let builder = flat("0xAbC0000000000000000000000000000000000001", 10);
    let outcome = |side| Asset::outcome(1234, side).unwrap();
    match name {
        "order_limit_gtc" => {
            Box::new(orders::order(&btc(), &limit(Side::Buy, "65000", "0.01", Tif::Gtc), None).unwrap())
        }
        "order_limit_alo_trailing_zeros" => {
            let sol = Market::perp(id(3), 2, 20, true).unwrap();
            Box::new(orders::order(&sol, &limit(Side::Sell, "2450.50", "1.200", Tif::Alo), None).unwrap())
        }
        "order_ioc_cloid_reduce_only" => {
            let eth = Market::perp(id(1), 4, 25, true).unwrap();
            let mut intent = limit(Side::Sell, "3100.5", "1.25", Tif::Ioc).with_cloid(cloid(1));
            intent.reduce_only = true;
            Box::new(orders::order(&eth, &intent, None).unwrap())
        }
        "order_trigger_sl_market" => {
            Box::new(orders::trigger(&btc(), &exit(Side::Sell, "0.02", Tpsl::StopLoss, "60000", None), None).unwrap())
        }
        "order_trigger_tp_limit" => {
            let avax = Market::perp(id(5), 0, 10, true).unwrap();
            let take = TriggerIntent::new(
                Side::Buy,
                Covers::Size(Amount::Size(dec("3"))),
                Tpsl::TakeProfit,
                Exit::limit(dec("150.5"), dec("151.25")),
            );
            Box::new(orders::trigger(&avax, &take, None).unwrap())
        }
        "order_hip4_frontend_market_buy" => {
            Box::new(PlaceOrder::one(Order::market(outcome(OutcomeSide::Yes), Side::Buy, dec("0.99999"), dec("25"))))
        }
        "order_hip4_frontend_market_sell" => {
            Box::new(PlaceOrder::one(Order::market(outcome(OutcomeSide::No), Side::Sell, dec("0.00001"), dec("10"))))
        }
        "order_hip3_builder" => {
            let xyz100 = Market::perp(Asset::hip3(1, 0).unwrap(), 2, 20, true).unwrap();
            Box::new(orders::order(&xyz100, &limit(Side::Buy, "6512.5", "0.1", Tif::Gtc), Some(&builder)).unwrap())
        }
        "order_two_orders" => Box::new(PlaceOrder::new(vec![
            Order::limit(id(0), Side::Buy, dec("64000"), dec("0.001"), Tif::Gtc),
            Order::limit(id(0), Side::Buy, dec("63000"), dec("0.002"), Tif::Gtc).with_cloid(Cloid::from_u128(0xabc)),
        ])),
        "order_vault_testnet" => {
            let two = Market::perp(id(2), 0, 20, true).unwrap();
            Box::new(orders::order(&two, &limit(Side::Sell, "0.25", "400", Tif::Gtc), None).unwrap())
        }
        "cancel" => Box::new(Cancel::new(vec![(id(0), 123_456_789), (outcome(OutcomeSide::Yes), 987_654_321_012)])),
        "update_leverage_cross" => Box::new(margin::leverage(&btc(), MarginMode::Cross, 20).unwrap()),
        "update_leverage_isolated_hip3" => {
            let xyz100 = Market::perp(Asset::hip3(1, 0).unwrap(), 2, 20, true).unwrap();
            Box::new(margin::leverage(&xyz100, MarginMode::Isolated, 5).unwrap())
        }
        "schedule_cancel_arm" => Box::new(ScheduleCancel::arm(now + 60_000, now).unwrap()),
        "schedule_cancel_disarm" => Box::new(ScheduleCancel::disarm()),
        "order_spot_frontend_market_builder" => {
            let hype = Market::spot(Asset::spot(107).unwrap(), 2).unwrap();
            let buy = OrderIntent::market(Side::Buy, Amount::Size(dec("0.5")), dec("40.829"), dec("0.01"));
            Box::new(orders::order(&hype, &buy, Some(&builder)).unwrap())
        }
        other => panic!("no typed builder for {other}"),
    }
}

#[test]
fn every_0_24_3_l1_case_is_built_by_a_typed_builder() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    assert_eq!(key.address().to_hex(), text(&file, "address").to_lowercase());
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 16);
    let now = cases[0].get("nonce").and_then(Json::as_u64).unwrap();
    for case in cases {
        let name = text(case, "name");
        check(name, l1_0_24_3(name, now).as_ref(), case, &key);
        // where another builder disarms with `time: null`, the SDK and this crate leave the key out
        if let Some(theirs) = case.get("reference") {
            assert_ne!(Some(text(theirs, "msgpack")), case.get("msgpack").and_then(Json::as_str), "{name}");
        }
    }
}
