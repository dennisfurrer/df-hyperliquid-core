//! What an account did, read back: a trader's fills window by window and an order in its parts,
//! how its orders ended and what they cost, as mainnet answered on 2026-10-08 at the pinned clock;
//! and another account's resting orders, found by the venue's id and by the client's.

use df_hyperliquid_core::account::history::{
    Fill, frontend_open_orders, historical_orders, open_orders, order_status, user_fees, user_fills,
    user_fills_by_time, user_twap_slice_fills,
};
use df_hyperliquid_core::account::{OrderKind, OrderLookup, OrderState};
use df_hyperliquid_core::markets::Dex;
use df_hyperliquid_core::trade::{Cloid, OrderRef, Tif, Tpsl};
use df_hyperliquid_core::{Client, Dec, Network};

use super::{address, bodies, dec};
use crate::support::venue::Venue;
use crate::support::{clock, fixture};

/// An account that closed three longs on 2026-10-08, the last a market sell of 4095 GRAM.
const TRADER: &str = "0xd80adf681abe30a3d644db1daae9abaa2d25a895";
/// An account with three triggers and a limit resting, every one under the same client id.
const WATCHED: &str = "0xf77a75616ff4aa16c890fabb4ab725a732d7e2b7";
/// The GRAM sell: its id and the millisecond it filled.
const SELL: u64 = 569_016_338_230;
const FILLED: u64 = 1_791_480_420_925;

fn by_time(start: u64, rest: &str) -> String {
    format!(r#"{{"type":"userFillsByTime","user":"{TRADER}","startTime":{start}{rest}}}"#)
}

fn sum(fills: &[Fill], figure: fn(&Fill) -> Dec) -> Dec {
    Dec::checked_sum(fills.iter().map(figure)).unwrap()
}

#[tokio::test]
async fn fills_read_window_by_window_come_as_whole_orders_and_in_their_parts() {
    let venue = Venue::mainnet()
        .info_where("userFillsByTime", &[("startTime", "1791456000000")], &fixture("account/fills_closed_mainnet.json"))
        .info_where(
            "userFillsByTime",
            &[("startTime", "1791460000001")],
            &fixture("trade/user_fills_by_time_aggregated.json"),
        )
        .info_where("userFillsByTime", &[("startTime", &FILLED.to_string())], &fixture("trade/user_fills_by_time.json"))
        .info("userFills", "trade/user_fills.json")
        .info("userTwapSliceFills", "trade/user_twap_slice_fills.json");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let trader = address(TRADER);

    let morning = user_fills_by_time(&client, trader, 1_791_456_000_000, Some(1_791_460_000_000), true).await.unwrap();
    let since = user_fills_by_time(&client, trader, 1_791_460_000_001, None, true).await.unwrap();
    let sell = &since[0];
    let parts = user_fills_by_time(&client, trader, sell.time, Some(sell.time), false).await.unwrap();
    let recent = user_fills(&client, trader, false).await.unwrap();
    let slices = user_twap_slice_fills(&client, trader).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            by_time(1_791_456_000_000, r#","endTime":1791460000000,"aggregateByTime":true"#),
            by_time(1_791_460_000_001, r#","aggregateByTime":true"#),
            by_time(FILLED, &format!(r#","endTime":{FILLED}"#)),
            format!(r#"{{"type":"userFills","user":"{TRADER}"}}"#),
            format!(r#"{{"type":"userTwapSliceFills","user":"{TRADER}"}}"#),
        ]
    );
    let closes: Vec<(&str, u64)> = morning.iter().chain(&since).map(|fill| (fill.coin.as_str(), fill.oid)).collect();
    assert_eq!(closes, [("NEAR", 568_540_814_915), ("SOL", 568_559_566_217), ("GRAM", SELL)], "oldest first");
    assert!(morning.iter().chain(&since).all(|fill| fill.dir == "Close Long" && fill.crossed));
    assert_eq!((sell.sz, sell.px), (dec("4095"), dec("1.3417402442")), "an average past the grid, kept exactly");
    assert_eq!(parts.len(), 14);
    assert!(parts.iter().all(|part| part.oid == SELL && part.time == FILLED));
    assert_eq!((sum(&parts, |part| part.sz), sum(&parts, |part| part.fee)), (sell.sz, sell.fee), "1.868099 either way");
    assert!(parts.iter().any(|part| part.tid == recent[0].tid), "the newest fill is one of its parts");
    assert_eq!(recent.last().map(|fill| fill.fee_token.as_str()), Some("UZEC"), "a spot buy pays in the token");
    assert_eq!(slices.iter().map(|slice| slice.twap_id).collect::<Vec<_>>(), [2_297_627, 2_297_621, 2_297_633]);
    assert!(slices.iter().all(|slice| slice.fill.hash == format!("0x{}", "0".repeat(64))), "a slice has no hash");
}

#[tokio::test]
async fn a_market_sell_is_followed_to_how_it_ended_and_the_rate_it_paid() {
    let venue = Venue::mainnet()
        .info("orderStatus", "trade/order_status_filled.json")
        .info("historicalOrders", "trade/historical_orders.json")
        .info("userFillsByTime", "trade/user_fills_by_time.json")
        .info("userFees", "trade/user_fees.json");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let trader = address(TRADER);

    let OrderLookup::Found(sell) = order_status(&client, trader, OrderRef::Oid(SELL)).await.unwrap() else {
        panic!("the sell is known")
    };
    let history = historical_orders(&client, trader).await.unwrap();
    let parts = user_fills_by_time(&client, trader, sell.status_timestamp, Some(sell.status_timestamp), false).await;
    let fees = user_fees(&client, trader).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [
            format!(r#"{{"type":"orderStatus","user":"{TRADER}","oid":{SELL}}}"#),
            format!(r#"{{"type":"historicalOrders","user":"{TRADER}"}}"#),
            by_time(FILLED, &format!(r#","endTime":{FILLED}"#)),
            format!(r#"{{"type":"userFees","user":"{TRADER}"}}"#),
        ]
    );
    assert_eq!((sell.status.clone(), sell.order.order_type.clone()), (OrderState::Filled, OrderKind::Market));
    assert_eq!(
        (sell.order.time_in_force(), sell.order.orig_sz, sell.order.sz),
        (Some(Tif::FrontendMarket), dec("4095"), Dec::ZERO)
    );
    let ends = |oid: u64| -> Vec<String> {
        history.iter().filter(|row| row.order.oid == oid).map(|row| row.status.to_string()).collect()
    };
    assert_eq!(ends(SELL), ["filled", "open"], "newest first");
    assert_eq!(ends(568_559_566_217), ["filled", "triggered", "open"], "the SOL close was a stop that fired");
    let rejected = history.iter().filter(|row| matches!(row.status, OrderState::Rejected(_))).count();
    assert_eq!(rejected, 2, "reduceOnlyRejected and minTradeNtlRejected");
    let liquidated = history.iter().find(|row| row.order.tif.as_deref() == Some("LiquidationMarket")).unwrap();
    assert_eq!(liquidated.order.time_in_force(), None, "a TIF only the venue sends is kept, not guessed");

    let parts = parts.unwrap();
    let notional = Dec::checked_sum(parts.iter().map(|part| part.sz.checked_mul(part.px).unwrap())).unwrap();
    let charged = notional.checked_mul(fees.user_cross_rate).unwrap();
    assert_eq!(fees.user_cross_rate, dec("0.00034"), "a taker under the 0.00045 base");
    let paid = sum(&parts, |part| part.fee);
    assert!(charged.checked_sub(paid).unwrap().abs() < dec("0.00001"), "{paid} paid, {charged} at the rate");
}

#[tokio::test]
async fn resting_orders_are_found_by_the_venue_s_id_and_by_the_client_s() {
    let cloid = "0xba5ed11067f2cc01ba5ed10000ba5ed1";
    let venue = Venue::mainnet()
        .info_where("openOrders", &[("dex", "xyz")], &fixture("account/open_orders_xyz_mainnet.json"))
        .info("openOrders", "trade/open_orders.json")
        .info("frontendOpenOrders", "trade/frontend_open_orders.json")
        .info_where("orderStatus", &[("oid", "569021780586")], &fixture("trade/order_status_open.json"))
        .info_where("orderStatus", &[("oid", cloid)], &fixture("trade/order_status_by_cloid.json"))
        .info("orderStatus", "trade/order_status_unknown.json");
    let client = Client::reader(venue, Network::Mainnet, clock);
    let watched = address(WATCHED);

    let resting = open_orders(&client, watched, Dex::Main).await.unwrap();
    let on_xyz = open_orders(&client, watched, Dex::Hip3("xyz")).await.unwrap();
    let kinds = frontend_open_orders(&client, watched, Dex::Main).await.unwrap();
    let stop = order_status(&client, watched, OrderRef::Oid(resting[0].oid)).await.unwrap();
    let tagged = order_status(&client, watched, OrderRef::Cloid(cloid.parse::<Cloid>().unwrap())).await.unwrap();
    let never = order_status(&client, watched, OrderRef::Oid(1)).await.unwrap();

    let user = format!(r#""user":"{WATCHED}""#);
    assert_eq!(
        bodies(client.http()),
        [
            format!(r#"{{"type":"openOrders",{user}}}"#),
            format!(r#"{{"type":"openOrders",{user},"dex":"xyz"}}"#),
            format!(r#"{{"type":"frontendOpenOrders",{user}}}"#),
            format!(r#"{{"type":"orderStatus",{user},"oid":569021780586}}"#),
            format!(r#"{{"type":"orderStatus",{user},"oid":"{cloid}"}}"#),
            format!(r#"{{"type":"orderStatus",{user},"oid":1}}"#),
        ]
    );
    assert_eq!(resting.iter().map(|order| order.coin.as_str()).collect::<Vec<_>>(), ["ENS", "@142", "HEMI"]);
    assert!(on_xyz.is_empty());
    let kinds: Vec<(OrderKind, Option<Tpsl>)> = kinds.iter().map(|row| (row.order_type.clone(), row.tpsl())).collect();
    assert_eq!(
        kinds,
        [
            (OrderKind::StopMarket, Some(Tpsl::StopLoss)),
            (OrderKind::Limit, None),
            (OrderKind::TakeProfitMarket, Some(Tpsl::TakeProfit))
        ]
    );
    let OrderLookup::Found(stop) = stop else { panic!("the stop rests") };
    assert_eq!((stop.status, stop.order.trigger()), (OrderState::Open, Some(dec("6.04"))));
    let OrderLookup::Found(tagged) = tagged else { panic!("the client id names an order") };
    assert_eq!((tagged.order.coin.as_str(), tagged.status), ("RENDER", OrderState::Filled));
    assert_eq!((tagged.order.trigger_condition.as_str(), tagged.order.trigger()), ("Triggered", None));
    assert_eq!(never, OrderLookup::Unknown);
}
