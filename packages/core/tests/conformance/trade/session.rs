//! The sessions `@nktkas/hyperliquid` 0.24.3 recorded, each L1 action signed by the session's agent:
//! `tpsl.json`, `leverage.json`, `session.json` and `outcomes.json`.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::trade::{
    Cancel, Close, Exit, Grouping, MarginMode, Market, Order, PlaceOrder, Side, Tif, Tpsl, margin, orders,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Asset, Json, OutcomeSide};

use super::{BUILDER, btc, check, cloid, dec, exit, flat, id, limit, market_buy, parse, text};

const TPSL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/tpsl.json"));
const LEVERAGE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/leverage.json"));
const SESSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/session.json"));
const OUTCOMES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcomes.json"));

/// Hardhat's account #1, the session vectors' agent: a published test key.
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn agent() -> LocalKey {
    LocalKey::from_hex(AGENT_KEY).unwrap()
}

/// `xyz:SP500`, the second perp on the first HIP-3 dex.
fn sp500() -> Market {
    Market::perp(Asset::hip3(1, 1).unwrap(), 2, 10, false).unwrap()
}

/// An entry with its take profit, its stop, and both: one action grouped
/// `normalTpsl`, each child a reduce-only market trigger the other way,
/// bounded 10% past its trigger as the venue's TP/SL page says.
#[test]
fn an_entry_with_its_exits_is_the_sdk_s_grouped_order() {
    let file = parse(TPSL);
    assert_eq!(agent().address().to_hex(), text(&file, "agent").to_lowercase());
    let builder = flat(BUILDER, 100);
    let cases = file.get("cases").unwrap();
    for (name, tp, sl) in [
        ("entry_tp", Some("90000"), None),
        ("entry_sl", None, Some("75000")),
        ("entry_tp_sl", Some("90000"), Some("75000")),
    ] {
        let mut entry = market_buy("0.00375", "80000", "0.005", 1);
        if let Some(at) = tp {
            entry = entry.take_profit(Exit::market(dec(at)));
        }
        if let Some(at) = sl {
            entry = entry.stop_loss(Exit::market(dec(at)));
        }
        let placed = orders::order(&btc(), &entry, Some(&builder)).unwrap();
        assert_eq!(placed.grouping(), Grouping::NormalTpsl, "{name}");
        let case = &cases.get(name).and_then(Json::as_array).unwrap()[0];
        check(name, &placed, case, &agent());
    }
}

/// The leverage sessions: the order alone, or `updateLeverage` first.
#[test]
fn the_leverage_cases_sign_the_same() {
    let file = parse(LEVERAGE);
    let builder = flat(BUILDER, 100);
    let order = |n| orders::order(&btc(), &market_buy("0.001", "80000", "0.005", n), Some(&builder)).unwrap();
    let mut count = 0;
    for (_, session) in file.get("cases").and_then(Json::as_object).unwrap() {
        for case in session.as_array().unwrap() {
            let name = text(case, "name");
            let typed: Box<dyn L1Action> = match name {
                "leverage" => Box::new(margin::leverage(&btc(), MarginMode::Cross, 5).unwrap()),
                "order" | "order_untouched" => Box::new(order(1)),
                "order_chosen_same" => Box::new(order(2)),
                other => panic!("no typed builder for {other}"),
            };
            check(name, typed.as_ref(), case, &agent());
            count += 1;
        }
    }
    assert_eq!(count, 7);
}

#[test]
fn the_first_session_signs_the_same() {
    let file = parse(SESSION);
    assert_eq!(agent().address().to_hex(), text(&file, "agent").to_lowercase());
    let builder = flat(BUILDER, 100);
    let routed = Some(&builder);
    let mut checked = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let typed: Box<dyn L1Action> = match name {
            // user-signed approvals, not L1 actions
            "approve_agent" | "approve_builder_fee" => continue,
            "btc_market_buy" => {
                Box::new(orders::order(&btc(), &market_buy("0.00375", "80000", "0.005", 1), routed).unwrap())
            }
            "btc_limit_sell" => {
                let sell = limit(Side::Sell, "81000", "0.00375", Tif::Gtc).with_cloid(cloid(2));
                Box::new(orders::order(&btc(), &sell, routed).unwrap())
            }
            "sp500_market_buy" => {
                Box::new(orders::order(&sp500(), &market_buy("0.1", "6712.5", "0.005", 3), routed).unwrap())
            }
            "sp500_limit_sell" => {
                let sell = limit(Side::Sell, "6800", "0.1", Tif::Gtc).with_cloid(cloid(4));
                Box::new(orders::order(&sp500(), &sell, routed).unwrap())
            }
            "btc_chase_alo" => {
                let chase = limit(Side::Buy, "79999", "0.001", Tif::Alo).with_cloid(cloid(5));
                Box::new(orders::order(&btc(), &chase, routed).unwrap())
            }
            "btc_take_profit" => Box::new(
                orders::trigger(&btc(), &exit(Side::Sell, "0.00375", Tpsl::TakeProfit, "90000", Some(6)), routed)
                    .unwrap(),
            ),
            "btc_stop_loss" => Box::new(
                orders::trigger(&btc(), &exit(Side::Sell, "0.00375", Tpsl::StopLoss, "75000", Some(7)), routed)
                    .unwrap(),
            ),
            "btc_close" => {
                let close = Close::new(dec("0.00375"), dec("80000"), dec("0.03")).with_cloid(cloid(8));
                Box::new(orders::close(&btc(), &close, routed).unwrap())
            }
            "cancel_one" => Box::new(Cancel::one(id(0), 1002)),
            "cancel_all" => Box::new(Cancel::new(vec![(sp500().asset(), 1003), (id(0), 1004)])),
            "sp500_leverage_isolated" => Box::new(margin::leverage(&sp500(), MarginMode::Isolated, 5).unwrap()),
            other => panic!("no typed builder for {other}"),
        };
        check(name, typed.as_ref(), case, &agent());
        checked += 1;
    }
    assert_eq!(checked, 11);
}

#[test]
fn the_outcome_session_s_orders_sign_the_same() {
    let file = parse(OUTCOMES);
    let builder = flat(BUILDER, 100).on(df_hyperliquid_core::utils::AssetKind::Outcome).unwrap();
    let yes = Asset::outcome(1473, OutcomeSide::Yes).unwrap();
    let no = Asset::outcome(1473, OutcomeSide::No).unwrap();
    let mut checked = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let typed: Box<dyn L1Action> = match name {
            "approve_agent_atlas" | "approve_builder_fee" => continue,
            "yes_market_buy" => {
                Box::new(PlaceOrder::one(Order::market(yes, Side::Buy, dec("0.612"), dec("16"))).through(builder))
            }
            "no_limit_buy" => Box::new(
                PlaceOrder::one(Order::limit(no, Side::Buy, dec("0.385"), dec("30"), Tif::Gtc)).through(builder),
            ),
            "yes_close" => {
                Box::new(PlaceOrder::one(Order::market(yes, Side::Sell, dec("0.5684"), dec("16"))).through(builder))
            }
            "no_cancel" => Box::new(Cancel::one(no, 2)),
            other => panic!("no typed builder for {other}"),
        };
        check(name, typed.as_ref(), case, &agent());
        checked += 1;
    }
    assert_eq!(checked, 4);
}
