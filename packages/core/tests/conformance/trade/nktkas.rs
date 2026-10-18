//! `nktkas-0.33.2-trade.json`: order vectors from `@nktkas/hyperliquid` 0.33.2 on both networks, the Python
//! SDK's signing tests, and hashes.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::trade::builder::fee::BuilderFee;
use df_hyperliquid_core::trade::{
    Amount, BatchModify, Builder, BuilderSetting, Cancel, Cloid, Covers, Exit, MarginMode, Market, Modify, Noop, Order,
    OrderRef, PlaceOrder, Scale, Side, Tif, Tpsl, TriggerIntent, Twap, TwapCancel, UpdateIsolatedMargin, margin,
    orders,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Asset, Json, OutcomeSide};

use super::{BUILDER, address, check, dec, id, limit, network, parse, text};

const TRADE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/nktkas-0.33.2-trade.json"));

fn paying_builder(order: Order, fee: u64) -> PlaceOrder {
    PlaceOrder::one(order).through(Builder::new(address(BUILDER), BuilderFee::from_tenths(fee)))
}

fn trigger(asset: u32, side: Side, price: &str, size: &str, at: &str, tpsl: Tpsl, is_market: bool) -> Order {
    Order::trigger(id(asset), side, dec(price), dec(size), dec(at), tpsl, is_market).reduce_only()
}

/// The 0.33.2 order vectors, each built through the typed builders and, where one fits, the ticket.
fn l1_0_33_2(name: &str) -> Box<dyn L1Action> {
    let at_88 = BuilderSetting::share_of_cap(address(BUILDER), 88).unwrap();
    let market_buy = || Order::market(id(0), Side::Buy, dec("86501"), dec("0.0012"));
    match name {
        "a_perp_market_buy_is_frontend_market_with_its_bound" => Box::new(PlaceOrder::one(market_buy())),
        "a_market_buy_through_the_terminal_s_builder_signs_as_the_sdk_does" => {
            Box::new(paying_builder(market_buy(), 0))
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
fn every_0_33_2_order_vector_signs_on_mainnet_and_testnet() {
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
            check(name, l1_0_33_2(name).as_ref(), case, &key);
            count += 1;
        }
    }
    assert_eq!(count, 50, "42 on both networks, 4 Python, 4 hashed");
    assert_eq!(networks, (21 + 2 + 4, 21 + 2));
}
