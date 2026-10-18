//! What accounts did, read from mainnet: the trader's fills, fees and orders, the watched account's
//! resting orders, and HLP's leader's ledger, over the windows the recordings were taken for where a
//! read takes one.

use df_hyperliquid_core::account::OrderLookup;
use df_hyperliquid_core::account::history::{
    frontend_open_orders, historical_orders, ledger, open_orders, order_status, user_fees, user_fills,
    user_fills_by_time, user_twap_slice_fills,
};
use df_hyperliquid_core::markets::Dex;
use df_hyperliquid_core::trade::OrderRef;

use super::{LEADER, UNIFIED, address};
use crate::tap::{reader, same_shape};

const TRADER: &str = "0xd80adf681abe30a3d644db1daae9abaa2d25a895";
const WATCHED: &str = "0xf77a75616ff4aa16c890fabb4ab725a732d7e2b7";
/// The trader's market sell of GRAM, and the millisecond it filled.
const SELL: u64 = 569_016_338_230;
const FILLED: u64 = 1_791_480_420_925;

#[tokio::test]
#[ignore = "mainnet"]
async fn the_trader_s_fills_read_by_window_and_by_twap_slice() {
    let client = reader();
    let trader = address(TRADER);
    let closes = user_fills_by_time(&client, trader, 1_791_456_000_000, Some(1_791_460_000_000), true).await.unwrap();
    assert_eq!(closes.len(), 2);
    same_shape(&client, "account/fills_closed_mainnet.json");
    let sold = user_fills_by_time(&client, trader, 1_791_460_000_001, Some(1_791_480_444_000), true).await.unwrap();
    assert_eq!(sold.iter().map(|fill| fill.oid).collect::<Vec<_>>(), [SELL]);
    same_shape(&client, "trade/user_fills_by_time_aggregated.json");
    assert_eq!(user_fills_by_time(&client, trader, FILLED, Some(FILLED), false).await.unwrap().len(), 14);
    same_shape(&client, "trade/user_fills_by_time.json");
    user_twap_slice_fills(&client, trader).await.unwrap();
    same_shape(&client, "trade/user_twap_slice_fills.json");
}

/// The 2000 most recent: whatever the trader has done since the recording, in its shape.
#[tokio::test]
#[ignore = "mainnet"]
async fn the_trader_s_latest_fills_read_split_and_aggregated() {
    let client = reader();
    let trader = address(TRADER);
    user_fills(&client, trader, false).await.unwrap();
    same_shape(&client, "trade/user_fills.json");
    user_fills(&client, trader, true).await.unwrap();
    same_shape(&client, "trade/user_fills.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn fees_read_for_a_trader_and_a_tiered_account() {
    let client = reader();
    user_fees(&client, address(TRADER)).await.unwrap();
    same_shape(&client, "trade/user_fees.json");
    user_fees(&client, address(UNIFIED)).await.unwrap();
    same_shape(&client, "account/fees_unified.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn orders_read_resting_by_either_id_and_in_history() {
    let client = reader();
    let (watched, trader) = (address(WATCHED), address(TRADER));
    open_orders(&client, watched, Dex::Main).await.unwrap();
    same_shape(&client, "trade/open_orders.json");
    open_orders(&client, watched, Dex::Hip3("xyz")).await.unwrap();
    same_shape(&client, "account/open_orders_xyz_mainnet.json");
    frontend_open_orders(&client, watched, Dex::Main).await.unwrap();
    same_shape(&client, "trade/frontend_open_orders.json");
    let sell = order_status(&client, trader, OrderRef::Oid(SELL)).await.unwrap();
    assert!(matches!(sell, OrderLookup::Found(found) if found.order.oid == SELL));
    same_shape(&client, "trade/order_status_filled.json");
    let cloid = "0xba5ed11067f2cc01ba5ed10000ba5ed1".parse().unwrap();
    order_status(&client, watched, OrderRef::Cloid(cloid)).await.unwrap();
    same_shape(&client, "trade/order_status_by_cloid.json");
    assert_eq!(order_status(&client, watched, OrderRef::Oid(1)).await.unwrap(), OrderLookup::Unknown);
    same_shape(&client, "trade/order_status_unknown.json");
    historical_orders(&client, trader).await.unwrap();
    same_shape(&client, "trade/historical_orders.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_leader_s_ledger_reads_by_window() {
    let client = reader();
    let leader = address(LEADER);
    assert_eq!(ledger(&client, leader, 1_683_243_000_000, Some(1_683_251_000_000)).await.unwrap().len(), 8);
    same_shape(&client, "account/ledger_hlp_made_mainnet.json");
    assert_eq!(ledger(&client, leader, 1_677_618_000_000, Some(1_677_810_000_000)).await.unwrap().len(), 3);
    same_shape(&client, "account/ledger_liquidated_mainnet.json");
    ledger(&client, leader, 1_786_838_806_142, None).await.unwrap();
    same_shape(&client, "account/ledger_since_mainnet.json");
}
