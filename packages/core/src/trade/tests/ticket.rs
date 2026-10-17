//! Each ticket check down to the exact JSON it would sign. The markets: BTC (asset 0, five size
//! decimals, 40x), ETH (1, four), `xyz:TSLA` (110001, three, 10x, isolated only), PURR/USDC (10000,
//! none) and `@107` (10107, two). A market order is bounded from 84000.5, one percent away.

use crate::trade::builder_fee::BuilderFee;
use crate::utils::sign::L1Action;
use crate::utils::{Address, Asset, Dec};

use super::*;
use crate::account::orders::FrontendOrder;
use crate::trade::builder::BuilderSetting;
use crate::trade::leverage::MarginMode;
use crate::trade::order::{Cloid, Side, Tif, Tpsl};

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn btc() -> Market {
    Market::perp(Asset::from_id(0), 5, 40, true).unwrap()
}

fn eth() -> Market {
    Market::perp(Asset::from_id(1), 4, 25, true).unwrap()
}

fn tsla() -> Market {
    Market::perp(Asset::from_id(110_001), 3, 10, false).unwrap()
}

fn hype() -> Market {
    Market::spot(Asset::from_id(10_107), 2).unwrap()
}

fn one_percent() -> Dec {
    dec("0.01")
}

fn market(side: Side, size: &str) -> OrderIntent {
    OrderIntent::market(side, Amount::Size(dec(size)), dec("84000.5"), one_percent())
}

fn limit(side: Side, price: &str, size: &str, tif: Tif) -> OrderIntent {
    OrderIntent::limit(side, Amount::Size(dec(size)), dec(price), tif)
}

fn wire(placed: Result<impl L1Action, Refusal>) -> String {
    placed.unwrap().wire()
}

fn terminal() -> Address {
    "0x6967d7AcD1EC5f210E9590666498e0cC5D13a843".parse().unwrap()
}

fn at_88_percent() -> BuilderSetting {
    BuilderSetting::share_of_cap(terminal(), 88).unwrap()
}

/// A resting order as `frontendOpenOrders` writes one, in the venue's own
/// words: `B` a buy, a stop with its `triggerPx`, a position's at `"0.0"`.
fn resting(oid: u64, side: &str, price: &str, size: &str, kind: &str, trigger: Option<&str>) -> FrontendOrder {
    let (is_trigger, trigger_px, tif, reduce) = match trigger {
        Some(at) => (true, at, "null", true),
        None => (false, "0.0", r#""Gtc""#, false),
    };
    serde_json::from_str(&format!(
        r#"{{"coin":"BTC","side":"{side}","limitPx":"{price}","sz":"{size}","oid":{oid},"timestamp":1790415847986,"triggerCondition":"N/A","isTrigger":{is_trigger},"triggerPx":"{trigger_px}","children":[],"isPositionTpsl":false,"reduceOnly":{reduce},"orderType":"{kind}","origSz":"{size}","tif":{tif},"cloid":null}}"#
    ))
    .unwrap()
}

#[test]
fn the_smallest_order_is_worth_the_venue_s_minimum() {
    assert_eq!(btc().grid().least_size(dec("84000")), Some(dec("0.00012")), "ten dollars at 84000, rounded up");
    let purr = Market::spot(Asset::from_id(10_000), 0).unwrap();
    assert_eq!(purr.grid().least_size(dec("0.12564")), Some(dec("80")));
}

#[test]
fn a_market_buy_is_frontend_market_bounded_above_the_mid() {
    assert_eq!(
        wire(order(&btc(), &market(Side::Buy, "0.00012"), None)),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"84840","s":"0.00012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
}

#[test]
fn a_market_sell_is_bounded_below_and_rounded_toward_the_mid() {
    assert_eq!(
        wire(order(&btc(), &market(Side::Sell, "0.00012"), None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"83161","s":"0.00012","r":false,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
}

/// A market order is `FrontendMarket` at a bound, never `Gtc`, and a limit cannot be made one by
/// its TIF.
#[test]
fn a_market_order_is_never_gtc_and_a_limit_is_never_frontend_market() {
    for side in [Side::Buy, Side::Sell] {
        let placed = order(&btc(), &market(side, "0.001"), None).unwrap();
        assert!(placed.orders().iter().all(|order| order.tif() == Some(Tif::FrontendMarket)));
        assert!(!placed.wire().contains("Gtc"));
    }
    assert_eq!(order(&btc(), &limit(Side::Buy, "84000", "0.001", Tif::FrontendMarket), None), Err(Refusal::MarketTif));
    assert_eq!(
        order(&btc(), &OrderIntent::market(Side::Buy, Amount::Size(dec("0.001")), dec("84000"), dec("0.51")), None),
        Err(Refusal::Trade(TradeError::Slippage(dec("0.51"))))
    );
}

#[test]
fn a_limit_is_brought_onto_the_grid_with_the_chosen_time_in_force() {
    let mut intent = limit(Side::Buy, "84000.7", "0.001", Tif::Alo);
    intent.reduce_only = true;
    assert_eq!(
        wire(order(&btc(), &intent, None)),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"84000","s":"0.001","r":true,"t":{"limit":{"tif":"Alo"}}}],"grouping":"na"}"#
    );
    // a sell rounds up: it never takes less than asked
    assert!(wire(order(&btc(), &limit(Side::Sell, "84000.7", "0.001", Tif::Gtc), None)).contains(r#""p":"84001""#));
}

#[test]
fn a_notional_is_cut_to_a_size_at_the_order_s_price() {
    let notional = OrderIntent::limit(Side::Buy, Amount::Notional(dec("100")), dec("84000"), Tif::Gtc);
    assert!(wire(order(&btc(), &notional, None)).contains(r#""s":"0.00119""#), "100 / 84000 = 0.0011904..., cut");
    let at_market = OrderIntent::market(Side::Sell, Amount::Notional(dec("100")), dec("84000.5"), one_percent());
    assert!(wire(order(&btc(), &at_market, None)).contains(r#""s":"0.00119""#), "a market order sizes at the mid");
}

#[test]
fn every_order_carries_the_builder_at_its_market_s_share_of_the_cap() {
    let builder = at_88_percent();
    let routed = order(&btc(), &limit(Side::Buy, "84000", "0.001", Tif::Gtc), Some(&builder)).unwrap();
    assert!(
        routed
            .wire()
            .ends_with(r#""grouping":"na","builder":{"b":"0x6967d7acd1ec5f210e9590666498e0cc5d13a843","f":88}}"#),
        "{}",
        routed.wire()
    );
    let spot = order(&hype(), &limit(Side::Buy, "44", "1", Tif::Gtc), Some(&builder)).unwrap();
    assert!(spot.wire().ends_with(r#""f":880}}"#), "{}", spot.wire());
}

/// One flat fee for every order: over a market's cap the order is refused with the reason, and the
/// fee is never lowered.
#[test]
fn a_builder_fee_over_the_cap_for_its_kind_is_refused_with_the_reason() {
    let flat = |fee: u64| BuilderSetting::flat(terminal(), BuilderFee::from_tenths(fee));
    let perp = limit(Side::Buy, "84000", "0.001", Tif::Gtc);
    assert!(order(&btc(), &perp, Some(&flat(100))).is_ok(), "0.1% is the perp cap itself");
    assert_eq!(
        order(&btc(), &perp, Some(&flat(101))).unwrap_err().to_string(),
        "the builder fee of 101 is over the 100 a perp order allows (0.1%)"
    );
    let hip3 = OrderIntent::market(Side::Buy, Amount::Size(dec("1")), dec("412.33"), one_percent());
    assert_eq!(
        order(&tsla(), &hip3, Some(&flat(500))).unwrap_err().to_string(),
        "the builder fee of 500 is over the 100 a HIP-3 perp order allows (0.1%)"
    );
    let spot = limit(Side::Buy, "44", "1", Tif::Gtc);
    assert!(order(&hype(), &spot, Some(&flat(1000))).is_ok(), "spot takes up to 1%");
    assert_eq!(
        order(&hype(), &spot, Some(&flat(1001))).unwrap_err().to_string(),
        "the builder fee of 1001 is over the 1000 a spot order allows (1%)"
    );
}

#[test]
fn a_stop_loss_at_market_bounds_itself_off_the_trigger() {
    let stop = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::StopLoss,
        Exit::market_within(dec("80000"), one_percent()),
    );
    assert_eq!(
        wire(trigger(&btc(), &stop, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"79200","s":"0.001","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"80000","tpsl":"sl"}}}],"grouping":"na"}"#
    );
}

#[test]
fn a_take_profit_can_rest_as_a_limit() {
    let take = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::TakeProfit,
        Exit::limit(dec("90000"), dec("89900")),
    );
    assert_eq!(
        wire(trigger(&btc(), &take, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"89900","s":"0.001","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"90000","tpsl":"tp"}}}],"grouping":"na"}"#
    );
}

/// The venue's TP/SL page: a market TP/SL takes 10% of slippage, which is
/// what [`Exit::market`] bounds it by.
#[test]
fn a_market_exit_takes_the_venue_s_ten_percent() {
    assert_eq!(TRIGGER_SLIPPAGE, dec("0.1"));
    let stop = TriggerIntent::new(Side::Sell, Covers::Position, Tpsl::StopLoss, Exit::market(dec("75000")));
    assert_eq!(
        wire(trigger(&btc(), &stop, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"67500","s":"0","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"positionTpsl"}"#,
        "the whole position: size 0, grouped positionTpsl"
    );
}

#[test]
fn a_trigger_rounds_to_the_nearest_tick_and_spot_takes_none() {
    let stop = TriggerIntent::new(
        Side::Sell,
        Covers::Size(Amount::Size(dec("0.001"))),
        Tpsl::StopLoss,
        Exit::market_within(dec("80000.6"), one_percent()),
    );
    assert!(wire(trigger(&btc(), &stop, None)).contains(r#""triggerPx":"80001""#));
    assert_eq!(trigger(&hype(), &stop, None), Err(Refusal::Spot("a take profit or stop loss")));
}

#[test]
fn an_entry_with_its_take_profit_and_stop_is_one_grouped_action() {
    let intent = OrderIntent::market(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), dec("0.005"))
        .with_cloid(Cloid::from_u128(0x7065_7270_0000_0000_0000_0000_0000_0001))
        .take_profit(Exit::market(dec("90000")))
        .stop_loss(Exit::market(dec("75000")));
    let placed = order(&btc(), &intent, None).unwrap();
    assert_eq!(placed.grouping(), crate::trade::order::Grouping::NormalTpsl);
    assert_eq!(
        placed.wire(),
        r#"{"type":"order","orders":[{"a":0,"b":true,"p":"80400","s":"0.00375","r":false,"t":{"limit":{"tif":"FrontendMarket"}},"c":"0x70657270000000000000000000000001"},{"a":0,"b":false,"p":"81000","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"90000","tpsl":"tp"}}},{"a":0,"b":false,"p":"67500","s":"0.00375","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"75000","tpsl":"sl"}}}],"grouping":"normalTpsl"}"#
    );
    let alone = OrderIntent::market(Side::Buy, Amount::Size(dec("0.00375")), dec("80000"), dec("0.005"));
    assert_eq!(order(&btc(), &alone, None).unwrap().grouping(), crate::trade::order::Grouping::Na, "no exit, no group");
}

#[test]
fn an_exit_on_the_wrong_side_of_the_entry_is_refused() {
    let long = limit(Side::Buy, "80000", "0.001", Tif::Gtc);
    assert_eq!(
        order(&btc(), &long.take_profit(Exit::market(dec("79000"))), None),
        Err(Refusal::ExitSide { tpsl: Tpsl::TakeProfit, trigger: dec("79000"), entry: dec("80000") })
    );
    assert_eq!(
        order(&btc(), &long.stop_loss(Exit::market(dec("81000"))), None),
        Err(Refusal::ExitSide { tpsl: Tpsl::StopLoss, trigger: dec("81000"), entry: dec("80000") })
    );
    let short = limit(Side::Sell, "80000", "0.001", Tif::Gtc).take_profit(Exit::market(dec("79000")));
    assert!(order(&btc(), &short, None).is_ok(), "a short's take profit is below");
}

#[test]
fn a_scale_splits_the_size_across_evenly_spaced_levels() {
    let leg = |price: &str| {
        format!(r#"{{"a":0,"b":true,"p":"{price}","s":"0.001","r":false,"t":{{"limit":{{"tif":"Gtc"}}}}}}"#)
    };
    let mut intent = Scale::new(Side::Buy, dec("0.003"), dec("83000"), dec("82000"), 3, Tif::Gtc);
    assert_eq!(
        wire(scale(&btc(), &intent, None)),
        format!(r#"{{"type":"order","orders":[{},{},{}],"grouping":"na"}}"#, leg("83000"), leg("82500"), leg("82000"))
    );
    intent.tif = Tif::Ioc;
    assert_eq!(scale(&btc(), &intent, None), Err(Refusal::ScaleTif(Tif::Ioc)));
    intent.tif = Tif::Gtc;
    for levels in [1, 51] {
        intent.levels = levels;
        assert_eq!(scale(&btc(), &intent, None), Err(Refusal::Levels(levels)));
    }
}

#[test]
fn a_twap_runs_between_five_minutes_and_a_day() {
    let mut intent = TwapIntent::new(Side::Buy, Amount::Size(dec("0.01")), 30).at(dec("84000.5"));
    assert_eq!(
        wire(twap(&btc(), &intent)),
        r#"{"type":"twapOrder","twap":{"a":0,"b":true,"s":"0.01","r":false,"m":30,"t":false}}"#
    );
    intent.minutes = 4;
    assert_eq!(twap(&btc(), &intent), Err(Refusal::Trade(TradeError::TwapMinutes(4))));
}

/// The venue's order types page: a TWAP totals at least $100.
#[test]
fn a_twap_worth_less_than_a_hundred_is_refused() {
    let small = TwapIntent::new(Side::Buy, Amount::Size(dec("0.001")), 30).at(dec("84000"));
    assert_eq!(
        twap(&btc(), &small),
        Err(Refusal::UnderMinimum { size: dec("0.001"), price: dec("84000"), minimum: MINIMUM_TWAP_VALUE })
    );
    let unpriced = TwapIntent::new(Side::Buy, Amount::Notional(dec("500")), 30);
    assert_eq!(twap(&btc(), &unpriced), Err(Refusal::NoReference));
}

#[test]
fn a_modify_replaces_the_chosen_order_in_place() {
    let order = resting(7, "B", "80000", "0.01", "Limit", None);
    let change = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Gtc };
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":7,"order":{"a":0,"b":true,"p":"80500","s":"0.02","r":false,"t":{"limit":{"tif":"Gtc"}}}}]}"#
    );
    let ioc = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Ioc };
    assert_eq!(modify(&btc(), &order, &ioc), Err(Refusal::ModifyTif(Tif::Ioc)), "without always place: Alo or Gtc");
}

#[test]
fn a_stop_is_modified_in_place_as_a_trigger_that_always_places() {
    let order = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let change = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Size(Amount::Size(dec("0.01"))),
    };
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":9,"order":{"a":0,"b":false,"p":"77220","s":"0.01","r":true,"t":{"trigger":{"isMarket":true,"triggerPx":"78000","tpsl":"sl"}}}}],"a":true}"#,
        "at market, bounded one percent past the new trigger, as a new stop is"
    );
}

#[test]
fn a_take_profit_modified_as_a_limit_is_brought_onto_the_grid() {
    let order = resting(11, "A", "89900", "0.01", "Take Profit Limit", Some("90000"));
    let change = Change::Trigger {
        trigger: dec("91000.57"),
        price: ExitPrice::Limit(dec("90950.77")),
        size: Covers::Size(Amount::Size(dec("0.01"))),
    };
    // the sell's limit rounds up, the trigger to the nearest tick
    assert_eq!(
        wire(modify(&btc(), &order, &change)),
        r#"{"type":"batchModify","modifies":[{"oid":11,"order":{"a":0,"b":false,"p":"90951","s":"0.01","r":true,"t":{"trigger":{"isMarket":false,"triggerPx":"91001","tpsl":"tp"}}}}],"a":true}"#
    );
}

#[test]
fn a_position_s_stop_keeps_size_zero_and_the_minimum_does_not_apply() {
    let order = resting(12, "A", "78705", "0.0", "Stop Market", Some("79500"));
    let whole = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Position,
    };
    assert!(wire(modify(&btc(), &order, &whole)).contains(r#""s":"0","r":true"#));
    let tiny = Change::Trigger {
        trigger: dec("78000"),
        price: ExitPrice::Market { slippage: one_percent() },
        size: Covers::Size(Amount::Size(dec("0.00001"))),
    };
    assert!(
        matches!(modify(&btc(), &order, &tiny), Err(Refusal::UnderMinimum { .. })),
        "a size typed in is held to the minimum as any order is"
    );
    let sized = resting(13, "A", "78705", "0.01", "Stop Market", Some("79500"));
    assert_eq!(modify(&btc(), &sized, &whole), Err(Refusal::NotWholePosition(13)));
}

#[test]
fn a_change_that_does_not_fit_the_order_is_refused() {
    let stop = resting(9, "A", "78705", "0.01", "Stop Market", Some("79500"));
    let limit_change = Change::Limit { price: dec("80500"), size: Amount::Size(dec("0.02")), tif: Tif::Gtc };
    assert_eq!(modify(&btc(), &stop, &limit_change), Err(Refusal::NotALimit(9)));
    let plain = resting(7, "B", "80000", "0.01", "Limit", None);
    let trigger_change =
        Change::Trigger { trigger: dec("78000"), price: ExitPrice::Limit(dec("78000")), size: Covers::Position };
    assert_eq!(modify(&btc(), &plain, &trigger_change), Err(Refusal::NotATrigger(7)));
    let trailing = resting(10, "A", "78705", "0.01", "Trailing Stop", Some("79500"));
    assert_eq!(
        modify(&btc(), &trailing, &trigger_change).unwrap_err().to_string(),
        "order 10 is a trailing stop trigger, which this build cannot modify"
    );
}

#[test]
fn closing_a_position_is_a_reduce_only_market_order_the_other_way() {
    let long = Close::new(dec("0.00375"), dec("80000"), dec("0.03"));
    assert_eq!(
        wire(close(&btc(), &long, None)),
        r#"{"type":"order","orders":[{"a":0,"b":false,"p":"77600","s":"0.00375","r":true,"t":{"limit":{"tif":"FrontendMarket"}}}],"grouping":"na"}"#
    );
    let short = Close::new(dec("-2"), dec("412.33"), one_percent());
    assert!(wire(close(&tsla(), &short, None)).contains(r#""b":true,"p":"416.45","s":"2","r":true"#));
    let half = Close { size: Some(dec("1")), ..short };
    assert!(wire(close(&tsla(), &half, None)).contains(r#""s":"1""#));
    let too_much = Close { size: Some(dec("3")), ..short };
    assert_eq!(close(&tsla(), &too_much, None), Err(Refusal::CloseTooLarge { size: dec("3"), position: dec("2") }));
    assert_eq!(close(&btc(), &Close::new(Dec::ZERO, dec("80000"), one_percent()), None), Err(Refusal::NoPosition));
    assert_eq!(close(&hype(), &long, None), Err(Refusal::Spot("a position")));
}

/// A whole position closes whatever it is worth; a part of one is an order
/// like any other.
#[test]
fn a_whole_close_goes_under_the_minimum_and_a_part_does_not() {
    let dust = Close::new(dec("0.00005"), dec("80000"), one_percent());
    assert!(close(&btc(), &dust, None).is_ok(), "four dollars of position, closed whole");
    let part = Close { size: Some(dec("0.00004")), ..dust };
    assert!(matches!(close(&btc(), &part, None), Err(Refusal::UnderMinimum { .. })));
}

#[test]
fn leverage_respects_the_market_s_ceiling_and_margin_mode() {
    assert_eq!(
        wire(leverage(&tsla(), MarginMode::Isolated, 10)),
        r#"{"type":"updateLeverage","asset":110001,"isCross":false,"leverage":10}"#
    );
    assert_eq!(leverage(&tsla(), MarginMode::Cross, 10), Err(Refusal::IsolatedOnly));
    assert_eq!(
        leverage(&tsla(), MarginMode::Isolated, 11).unwrap_err().to_string(),
        "leverage is a whole number from 1 to 10, not 11"
    );
    assert_eq!(leverage(&tsla(), MarginMode::Isolated, 0), Err(Refusal::Leverage { asked: 0, most: 10 }));
    assert_eq!(leverage(&hype(), MarginMode::Cross, 2), Err(Refusal::Spot("leverage")));
}

#[test]
fn isolated_margin_moves_millionths_on_the_position_s_side() {
    let short = dec("-2");
    assert_eq!(
        wire(margin(&tsla(), short, MarginMode::Isolated, dec("12.5"))),
        r#"{"type":"updateIsolatedMargin","asset":110001,"isBuy":false,"ntli":12500000}"#
    );
    assert_eq!(
        wire(margin(&tsla(), short, MarginMode::Isolated, dec("-12.5"))),
        r#"{"type":"updateIsolatedMargin","asset":110001,"isBuy":false,"ntli":-12500000}"#
    );
    assert_eq!(margin(&tsla(), dec("2"), MarginMode::Cross, dec("12.5")), Err(Refusal::CrossPosition));
    assert_eq!(margin(&tsla(), Dec::ZERO, MarginMode::Isolated, dec("12.5")), Err(Refusal::NoPosition));
}

#[test]
fn spot_takes_no_reduce_only_and_its_prices_keep_eight_places() {
    assert_eq!(
        wire(order(&hype(), &limit(Side::Buy, "44.1234567", "1.239", Tif::Ioc), None)),
        r#"{"type":"order","orders":[{"a":10107,"b":true,"p":"44.123","s":"1.23","r":false,"t":{"limit":{"tif":"Ioc"}}}],"grouping":"na"}"#
    );
    let mut reducing = limit(Side::Buy, "44", "1", Tif::Gtc);
    reducing.reduce_only = true;
    // reduce only on spot is refused rather than dropped
    assert_eq!(order(&hype(), &reducing, None), Err(Refusal::Spot("reduce only")));
}

#[test]
fn an_order_under_the_venue_s_minimum_is_refused_here_first() {
    let refused = order(&btc(), &limit(Side::Buy, "84000", "0.0001", Tif::Gtc), None).unwrap_err();
    assert_eq!(refused, Refusal::UnderMinimum { size: dec("0.0001"), price: dec("84000"), minimum: dec("10") });
    assert_eq!(refused.to_string(), "0.0001 at 84000 is worth 8.4, under the venue's minimum of 10");
    assert_eq!(
        order(&btc(), &limit(Side::Buy, "84000", "0.000001", Tif::Gtc), None),
        Err(Refusal::Trade(TradeError::NoSize(dec("0.000001"))))
    );
    assert_eq!(
        order(&btc(), &limit(Side::Buy, "0", "1", Tif::Gtc), None),
        Err(Refusal::NotPositive { what: "a price", value: Dec::ZERO })
    );
}

#[test]
fn prices_and_sizes_never_pass_through_a_float() {
    // 0.1 + 0.2 is 0.3 exactly, and so is what is signed
    assert_eq!(dec("0.1").checked_add(dec("0.2")), Some(dec("0.3")));
    assert!(
        wire(order(&eth(), &limit(Side::Buy, "0.3", "33.3334", Tif::Gtc), None)).contains(r#""p":"0.3","s":"33.3334""#)
    );
}

#[test]
fn a_market_is_only_the_kind_its_asset_id_names() {
    assert!(Market::perp(Asset::from_id(10_107), 2, 3, true).is_none(), "a spot id is not a perp");
    assert!(Market::spot(Asset::from_id(0), 2).is_none(), "a perp id is not a pair");
    assert!(Market::perp(Asset::from_id(100_012_100), 0, 1, true).is_none(), "an outcome is an outcome market's");
    assert!(Market::perp(Asset::from_id(110_001), 3, 10, false).is_some_and(|market| !market.allows_cross()));
}
