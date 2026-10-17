//! Every order-family L1 vector on file, rebuilt through the typed builders rather than read back as a map:
//! the same action JSON, msgpack, hash, and signature on the network the case was made for. Where a file
//! records the msgpack it is compared as written; where it does not, the typed action's msgpack is compared
//! with this crate's reading of the recorded JSON. Each file names the implementation that made it:
//!
//! - `l1.json`: `@nktkas/hyperliquid` 0.24.3.
//! - `tpsl.json`, `leverage.json`, `desk.json`, `outcomes.json`: desk and outcome sessions by the same SDK,
//!   signed by the desk's agent.
//! - `vault.json`: the venue's Python SDK 0.19.0, acting for a subaccount.
//! - `nktkas-0.33.2-trade.json`: order vectors from `@nktkas/hyperliquid` 0.33.2 on both networks, the Python
//!   SDK's signing tests, and hashes.

// A test crate: clippy's allowance for tests covers `#[test]` functions only, not their helpers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::trade::builder::fee::BuilderFee;
use df_hyperliquid_core::trade::{
    Amount, BatchModify, Builder, BuilderSetting, Cancel, Cloid, Close, Covers, Exit, Grouping, MarginMode, Market,
    Modify, Noop, Order, OrderIntent, OrderRef, PlaceOrder, Scale, ScheduleCancel, Side, Tif, Tpsl, TriggerIntent,
    Twap, TwapCancel, UpdateIsolatedMargin, margin, orders,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Address, Asset, Dec, Json, OutcomeSide, hex};

const L1: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/l1.json"));
const TPSL: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/tpsl.json"));
const LEVERAGE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/leverage.json"));
const DESK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/desk.json"));
const OUTCOMES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/outcomes.json"));
const VAULT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/vault.json"));
const TRADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2-trade.json"));

/// Hardhat's account #1, the desk vectors' agent: a published test key.
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// The builder the desk and 0.33.2 vectors name.
const TERMINAL: &str = "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843";

fn parse(text: &str) -> Json {
    Json::parse(text).unwrap()
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn id(id: u32) -> Asset {
    Asset::from_id(id)
}

fn cloid(n: u128) -> Cloid {
    Cloid::from_u128((0x7065_7270_u128 << 96) | n)
}

fn flat(builder: &str, fee: u64) -> BuilderSetting {
    BuilderSetting::flat(address(builder), BuilderFee::from_tenths(fee))
}

fn network(case: &Json) -> Network {
    match case.get("isTestnet") {
        Some(Json::Bool(true)) => Network::Testnet,
        Some(Json::Bool(false)) | None => Network::Mainnet,
        other => panic!("isTestnet is {other:?}"),
    }
}

/// The Python SDK writes `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

fn check(name: &str, typed: &dyn L1Action, case: &Json, key: &LocalKey) {
    let recorded = case.get("action").unwrap();
    assert_eq!(typed.wire(), recorded.compact(), "{name}: the action");
    assert_eq!(typed.msgpack(), recorded.to_action().unwrap().msgpack(), "{name}: msgpack of the recorded JSON");
    if let Some(Json::String(packed)) = case.get("msgpack") {
        assert_eq!(hex::encode_prefixed(&typed.msgpack()), *packed, "{name}: msgpack");
    }
    let nonce = case.get("nonce").and_then(Json::as_u64).unwrap();
    let vault = match case.get("vaultAddress") {
        Some(Json::String(vault)) => Some(address(vault)),
        _ => None,
    };
    if let Some(Json::String(hash)) = case.get("actionHash") {
        assert_eq!(hex::encode_prefixed(&typed.connection_id(nonce, vault)), *hash, "{name}: hash");
    }
    let Some(expected) = case.get("signature") else { return };
    let network = network(case);
    let signature = key.sign_digest(&typed.digest(nonce, vault, network)).unwrap();
    assert_eq!(signature.r_hex(), word(text(expected, "r")), "{name} on {network}: r");
    assert_eq!(signature.s_hex(), word(text(expected, "s")), "{name} on {network}: s");
    assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{name}: v");
    // the recorded body leaves out an absent vaultAddress; this crate writes it null, which the venue reads alike
    if let Some(Json::String(body)) = case.get("body") {
        let ours = typed.body(nonce, &signature, vault);
        assert_eq!(ours, format!("{},\"vaultAddress\":null}}", &body[..body.len() - 1]), "{name}: body");
    }
}

fn agent() -> LocalKey {
    LocalKey::from_hex(AGENT_KEY).unwrap()
}

fn btc() -> Market {
    Market::perp(id(0), 5, 40, true).unwrap()
}

/// `xyz:SP500`, the second perp on the first HIP-3 dex.
fn sp500() -> Market {
    Market::perp(Asset::hip3(1, 1).unwrap(), 2, 10, false).unwrap()
}

fn market_buy(size: &str, reference: &str, slippage: &str, n: u128) -> OrderIntent {
    OrderIntent::market(Side::Buy, Amount::Size(dec(size)), dec(reference), dec(slippage)).with_cloid(cloid(n))
}

fn limit(side: Side, price: &str, size: &str, tif: Tif) -> OrderIntent {
    OrderIntent::limit(side, Amount::Size(dec(size)), dec(price), tif)
}

fn exit(side: Side, size: &str, tpsl: Tpsl, at: &str, n: Option<u128>) -> TriggerIntent {
    let intent = TriggerIntent::new(side, Covers::Size(Amount::Size(dec(size))), tpsl, Exit::market(dec(at)));
    match n {
        Some(n) => intent.with_cloid(cloid(n)),
        None => intent,
    }
}

fn atlas_l1(name: &str, now: u64) -> Box<dyn L1Action> {
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
fn every_atlas_l1_case_is_built_by_a_typed_builder() {
    let file = parse(L1);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    assert_eq!(key.address().to_hex(), text(&file, "address").to_lowercase());
    let cases = file.get("cases").and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), 16);
    let now = cases[0].get("nonce").and_then(Json::as_u64).unwrap();
    for case in cases {
        let name = text(case, "name");
        check(name, atlas_l1(name, now).as_ref(), case, &key);
        // where another builder disarms with `time: null`, the SDK and this crate leave the key out
        if let Some(theirs) = case.get("reference") {
            assert_ne!(Some(text(theirs, "msgpack")), case.get("msgpack").and_then(Json::as_str), "{name}");
        }
    }
}

/// An entry with its take profit, its stop, and both: one action grouped
/// `normalTpsl`, each child a reduce-only market trigger the other way,
/// bounded 10% past its trigger as the venue's TP/SL page says.
#[test]
fn an_entry_with_its_exits_is_the_sdk_s_grouped_order() {
    let file = parse(TPSL);
    assert_eq!(agent().address().to_hex(), text(&file, "agent").to_lowercase());
    let builder = flat(TERMINAL, 100);
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

/// The desk's leverage sessions: the order alone, or `updateLeverage` first.
#[test]
fn the_desk_s_leverage_cases_sign_the_same() {
    let file = parse(LEVERAGE);
    let builder = flat(TERMINAL, 100);
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
fn the_desk_s_first_session_signs_the_same() {
    let file = parse(DESK);
    assert_eq!(agent().address().to_hex(), text(&file, "agent").to_lowercase());
    let builder = flat(TERMINAL, 100);
    let routed = Some(&builder);
    let mut checked = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let typed: Box<dyn L1Action> = match name {
            // user-signed approvals, not L1 actions
            "approve_agent_atlas" | "approve_builder_fee" => continue,
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
    let builder = flat(TERMINAL, 100).on(df_hyperliquid_core::utils::AssetKind::Outcome).unwrap();
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

/// Python SDK 0.19.0 on a subaccount: an entry, its stop and a cancel, each
/// hashed with the subaccount as the vault and signed by its master's agent.
#[test]
fn the_python_sdk_s_subaccount_orders_sign_the_same() {
    let file = parse(VAULT);
    let mut checked = 0;
    for case in file.get("cases").and_then(Json::as_array).unwrap() {
        let name = text(case, "name");
        let key = LocalKey::from_hex(text(case, "key")).unwrap();
        assert_eq!(key.address().to_hex(), text(case, "address").to_lowercase(), "{name}");
        let typed: Box<dyn L1Action> = match name {
            // a dummy action, not an order
            "sdk_test_dummy" => continue,
            "order_market_entry" => {
                Box::new(orders::order(&btc(), &market_buy("0.00118", "84100.5", "0.005", 1), None).unwrap())
            }
            "order_stop_market" => Box::new(
                orders::trigger(&btc(), &exit(Side::Sell, "0.00118", Tpsl::StopLoss, "77372", Some(2)), None).unwrap(),
            ),
            "cancel_stop" => Box::new(Cancel::one(id(0), 1002)),
            other => panic!("no typed builder for {other}"),
        };
        check(name, typed.as_ref(), case, &key);
        checked += 1;
    }
    assert_eq!(checked, 3);
}

fn terminal_paying(order: Order, fee: u64) -> PlaceOrder {
    PlaceOrder::one(order).through(Builder::new(address(TERMINAL), BuilderFee::from_tenths(fee)))
}

fn trigger(asset: u32, side: Side, price: &str, size: &str, at: &str, tpsl: Tpsl, is_market: bool) -> Order {
    Order::trigger(id(asset), side, dec(price), dec(size), dec(at), tpsl, is_market).reduce_only()
}

/// The 0.33.2 order vectors, each built through the typed builders and, where one fits, the ticket.
fn hip4_tui(name: &str) -> Box<dyn L1Action> {
    let at_88 = BuilderSetting::share_of_cap(address(TERMINAL), 88).unwrap();
    let market_buy = || Order::market(id(0), Side::Buy, dec("86501"), dec("0.0012"));
    match name {
        "a_perp_market_buy_is_frontend_market_with_its_bound" => Box::new(PlaceOrder::one(market_buy())),
        "a_market_buy_through_the_terminal_s_builder_signs_as_the_sdk_does" => {
            Box::new(terminal_paying(market_buy(), 0))
        }
        "a_hip3_post_only_carries_its_cloid_and_the_builder_last" => {
            let order = Order::limit(id(110_003), Side::Sell, dec("340.18"), dec("2.5"), Tif::Alo)
                .reduce_only()
                .with_cloid(Cloid::from_u128(0xabcd));
            Box::new(PlaceOrder::one(order).through(Builder::new(
                address("0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704"),
                BuilderFee::from_tenths(10),
            )))
        }
        "a_spot_order_names_ten_thousand_and_the_pair_index" => {
            let hype = Market::spot(id(10_107), 2).unwrap();
            Box::new(orders::order(&hype, &limit(Side::Buy, "44.123", "1.25", Tif::Ioc), None).unwrap())
        }
        "a_stop_loss_at_market_writes_is_market_then_the_trigger_then_the_kind" => {
            Box::new(PlaceOrder::one(trigger(1, Side::Sell, "2900", "0.5", "3000", Tpsl::StopLoss, true)))
        }
        "a_take_profit_at_a_limit" => {
            let eth = Market::perp(id(1), 4, 25, true).unwrap();
            let take = TriggerIntent::new(
                Side::Sell,
                Covers::Size(Amount::Size(dec("0.5"))),
                Tpsl::TakeProfit,
                Exit::limit(dec("3490"), dec("3500")),
            );
            Box::new(orders::trigger(&eth, &take, None).unwrap())
        }
        "a_scale_is_one_action_with_a_leg_per_level" => {
            let five = Market::perp(id(5), 2, 10, true).unwrap();
            let levels = Scale::new(Side::Buy, dec("3"), dec("150"), dec("145"), 3, Tif::Gtc);
            Box::new(orders::scale(&five, &levels, None).unwrap())
        }
        "a_cancel_names_asset_and_order" => Box::new(Cancel::new(vec![(id(0), 123_456_789), (id(110_003), 42)])),
        "a_batch_modify_puts_the_order_id_before_the_new_order" => Box::new(BatchModify::new(vec![Modify::new(
            OrderRef::Oid(987_654_321),
            Order::limit(id(0), Side::Buy, dec("84000"), dec("0.01"), Tif::Gtc),
        )])),
        "a_batch_modify_of_a_stop_at_market_ends_with_always_place" => Box::new(BatchModify::new(vec![Modify::new(
            OrderRef::Oid(987_654_321),
            trigger(0, Side::Sell, "79200", "0.001", "80000", Tpsl::StopLoss, true),
        )])),
        "a_batch_modify_of_a_hip3_take_profit_at_a_limit" => Box::new(BatchModify::new(vec![Modify::new(
            OrderRef::Oid(42),
            trigger(110_003, Side::Buy, "380.5", "2.5", "381", Tpsl::TakeProfit, false),
        )])),
        "a_batch_modify_of_a_position_stop_keeps_size_zero" => Box::new(BatchModify::new(vec![Modify::new(
            OrderRef::Oid(7),
            trigger(1, Side::Sell, "2900", "0", "3000", Tpsl::StopLoss, true),
        )])),
        "a_twap_nests_its_fields_under_twap" => {
            Box::new(Twap::new(id(0), Side::Buy, dec("0.5"), 30).unwrap().randomized())
        }
        "a_twap_cancel_is_flat" => Box::new(TwapCancel::new(id(0), 1234)),
        "leverage_is_asset_then_cross_then_the_figure" => {
            let tsla = Market::perp(id(110_003), 3, 10, false).unwrap();
            Box::new(margin::leverage(&tsla, MarginMode::Isolated, 10).unwrap())
        }
        "margin_goes_in_as_millionths" => Box::new(UpdateIsolatedMargin::new(id(1), Side::Buy, dec("12.5")).unwrap()),
        "margin_comes_out_as_a_negative_int32" => {
            let eth = Market::perp(id(1), 4, 25, true).unwrap();
            Box::new(margin::margin(&eth, dec("1"), MarginMode::Isolated, dec("-2.5")).unwrap())
        }
        "an_order_at_each_market_s_fee_signs_as_the_sdk_does_perp" => Box::new(
            PlaceOrder::one(market_buy()).through(at_88.on(df_hyperliquid_core::utils::AssetKind::Perp).unwrap()),
        ),
        "an_order_at_each_market_s_fee_signs_as_the_sdk_does_hip3" => {
            let tsla = Market::perp(id(110_001), 3, 10, false).unwrap();
            Box::new(orders::order(&tsla, &limit(Side::Sell, "340.18", "2.5", Tif::Gtc), Some(&at_88)).unwrap())
        }
        "an_order_at_each_market_s_fee_signs_as_the_sdk_does_spot" => {
            let hype = Market::spot(id(10_107), 2).unwrap();
            Box::new(orders::order(&hype, &limit(Side::Buy, "44", "1", Tif::Gtc), Some(&at_88)).unwrap())
        }
        "an_order_at_each_market_s_fee_signs_as_the_sdk_does_outcome" => {
            let edge = Order::market(id(100_012_100), Side::Buy, dec("0.99999"), dec("20"));
            Box::new(PlaceOrder::one(edge).through(at_88.on(df_hyperliquid_core::utils::AssetKind::Outcome).unwrap()))
        }
        "the_python_sdk_order_vector_signs_the_same" => {
            Box::new(PlaceOrder::one(Order::limit(id(1), Side::Buy, dec("100"), dec("100"), Tif::Gtc)))
        }
        "the_python_sdk_trigger_vector_signs_the_same" => Box::new(PlaceOrder::one(Order::trigger(
            id(1),
            Side::Buy,
            dec("100"),
            dec("100"),
            dec("103"),
            Tpsl::StopLoss,
            true,
        ))),
        "the_order_action_hashes_to_the_verified_value" => {
            let yes = Asset::outcome(516, OutcomeSide::Yes).unwrap();
            Box::new(PlaceOrder::one(Order::market(yes, Side::Buy, dec("0.35"), dec("50"))))
        }
        "the_cancel_action_hashes_to_the_verified_value" => {
            Box::new(Cancel::one(Asset::outcome(516, OutcomeSide::Yes).unwrap(), 123_456_789))
        }
        "the_noop_action_hashes_to_what_the_other_implementation_gets"
        | "acting_as_a_subaccount_is_the_vault_marker_and_changes_the_hash" => Box::new(Noop),
        other => panic!("no typed builder for {other}"),
    }
}

#[test]
fn every_hip4_tui_order_vector_signs_on_mainnet_and_testnet() {
    let file = parse(TRADE);
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let mut networks = (0, 0);
    let mut count = 0;
    for list in ["l1", "python", "hashed"] {
        for case in file.get(list).and_then(Json::as_array).unwrap() {
            let name = text(case, "name");
            match network(case) {
                Network::Mainnet => networks.0 += 1,
                Network::Testnet => networks.1 += 1,
            }
            check(name, hip4_tui(name).as_ref(), case, &key);
            count += 1;
        }
    }
    assert_eq!(count, 50, "42 on both networks, 4 Python, 4 hashed");
    assert_eq!(networks, (21 + 2 + 4, 21 + 2));
}
