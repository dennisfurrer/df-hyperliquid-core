//! What is listed and what it trades at, read from mainnet: every `/info` read of
//! [`df_hyperliquid_core::markets`], each held against its recording right after it is read.

use df_hyperliquid_core::client::native::clock;
use df_hyperliquid_core::markets::dexes::{self, Dexes};
use df_hyperliquid_core::markets::outcomes::{self, Book as OutcomeBook};
use df_hyperliquid_core::markets::{self, Dex, Interval, Precision, candle, mids, perps, spot, trades};
use df_hyperliquid_core::utils::{Address, OutcomeSide};

use crate::tap::{reader, same_shape};

const HOUR: u64 = 3_600_000;

/// HLP Strategy A, whose funding is recorded.
fn vault() -> Address {
    "0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap()
}

#[tokio::test]
#[ignore = "mainnet"]
async fn perp_meta_lists_btc() {
    let client = reader();
    let meta = perps::meta(&client, Dex::Main).await.unwrap().unwrap();
    assert!(meta.find(0, "BTC").is_some());
    same_shape(&client, "perps/meta_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_btc_book_has_both_sides() {
    let client = reader();
    let book = markets::book::l2_book(&client, "BTC", Precision::FULL).await.unwrap();
    let (bid, ask) = book.touch();
    assert!(bid.unwrap() < ask.unwrap());
    same_shape(&client, "trade/l2_book.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_btc_book_reads_bucketed_at_three_and_at_five_figures() {
    let client = reader();
    let three = Precision::figures(3).unwrap();
    let book = markets::book::l2_book(&client, "BTC", three).await.unwrap();
    assert!(book.spread.is_some());
    same_shape(&client, "trade/l2_book_three_figures.json");
    let step_two = Precision::figures(5).and_then(|five| five.step(2)).unwrap();
    let book = markets::book::l2_book(&client, "BTC", step_two).await.unwrap();
    assert!(book.spread.is_some());
    same_shape(&client, "trade/l2_book_five_figures_step_two.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_main_dex_lists_each_perp_with_its_context_and_its_cap() {
    let client = reader();
    let listed = perps::meta_and_asset_ctxs(&client, Dex::Main).await.unwrap().unwrap();
    assert_eq!(listed.ctxs.len(), listed.meta.universe.len());
    assert!(listed.find(0, "BTC").is_some_and(|(_, _, ctx)| ctx.mid_px.is_some()));
    same_shape(&client, "perps/meta_and_asset_ctxs_mainnet.json");
    let capped = perps::perps_at_open_interest_cap(&client, Dex::Main).await.unwrap();
    assert!(capped.iter().all(|coin| listed.find(0, coin).is_some()), "{capped:?}");
    same_shape(&client, "perps/perps_at_open_interest_cap_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn every_hip3_dex_lines_up_with_its_meta() {
    let client = reader();
    let list = dexes::perp_dexs(&client).await.unwrap();
    assert_eq!(list.index_of("xyz"), Some(1));
    same_shape(&client, "hip3/perp_dexs_mainnet.json");
    let metas = dexes::all_perp_metas(&client).await.unwrap();
    same_shape(&client, "hip3/all_perp_metas_mainnet.json");
    let joined = Dexes::new(list, metas).unwrap();
    assert_eq!(joined.find("xyz:TSLA").map(|(asset, _)| asset.id()), Some(110_001));
}

#[tokio::test]
#[ignore = "mainnet"]
async fn a_hip3_dex_reads_its_meta_contexts_limits_status_mids_and_book() {
    let client = reader();
    let xyz = Dex::Hip3("xyz");
    let meta = perps::meta(&client, xyz).await.unwrap().unwrap();
    assert!(meta.find(1, "xyz:TSLA").is_some());
    same_shape(&client, "hip3/meta_xyz_mainnet.json");
    let limits = dexes::perp_dex_limits(&client, xyz).await.unwrap().unwrap();
    assert!(limits.coin_cap("xyz:TSLA").is_some());
    same_shape(&client, "hip3/perp_dex_limits_xyz_mainnet.json");
    let status = dexes::perp_dex_status(&client, xyz).await.unwrap().unwrap();
    assert!(status.total_net_deposit.is_positive());
    same_shape(&client, "hip3/perp_dex_status_xyz_mainnet.json");
    let prices = mids::all_mids(&client, xyz).await.unwrap();
    assert!(prices.get("xyz:TSLA").is_some());
    same_shape(&client, "trade/all_mids_xyz.json");
    let para = perps::meta_and_asset_ctxs(&client, Dex::Hip3("para")).await.unwrap().unwrap();
    assert!(para.find(8, "para:10Y").is_some());
    same_shape(&client, "hip3/meta_and_asset_ctxs_para_mainnet.json");
    let book = markets::book::l2_book(&client, "para:10Y", Precision::FULL).await.unwrap();
    assert_eq!(book.coin, "para:10Y");
    same_shape(&client, "hip3/l2_book_para_10y_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn btc_s_candles_prints_and_mid_read() {
    let client = reader();
    let now = clock();
    let candles = candle::candle_snapshot(&client, "BTC", Interval::OneHour, now - 4 * HOUR, now).await.unwrap();
    assert!(candles.windows(2).all(|pair| pair[0].close_time + 1 == pair[1].open_time));
    same_shape(&client, "trade/candle_snapshot.json");
    let prints = trades::recent_trades(&client, "BTC").await.unwrap();
    assert!(prints.iter().all(|print| print.coin == "BTC"));
    same_shape(&client, "trade/recent_trades.json");
    let prices = mids::all_mids(&client, Dex::Main).await.unwrap();
    assert!(prices.get("BTC").is_some() && prices.get("@107").is_some());
    same_shape(&client, "trade/all_mids.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn funding_reads_settled_paid_and_predicted() {
    let client = reader();
    let now = clock();
    let settled = perps::funding_history_range(&client, "BTC", now - 3 * HOUR, now).await.unwrap();
    assert!(settled.iter().all(|rate| rate.coin == "BTC") && !settled.is_empty());
    same_shape(&client, "perps/funding_history_btc_mainnet.json");
    let paid = perps::user_funding_range(&client, vault(), now - HOUR, now, 1).await.unwrap();
    assert!(paid.iter().all(|row| row.delta.kind == "funding"));
    same_shape(&client, "perps/user_funding_mainnet.json");
    let predicted = perps::predicted_fundings(&client).await.unwrap();
    assert_eq!(predicted.hyperliquid("BTC").and_then(|next| next.funding_interval_hours), Some(1));
    same_shape(&client, "perps/predicted_fundings_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn spot_lists_its_pairs_with_their_contexts_and_a_token_s_details() {
    let client = reader();
    let meta = spot::spot_meta(&client).await.unwrap();
    let hype = meta.market_of("HYPE", "USDC").unwrap();
    assert_eq!(hype.asset().map(|asset| asset.id()), Some(10_107));
    same_shape(&client, "spot/spot_meta_mainnet.json");
    let listed = spot::spot_meta_and_asset_ctxs(&client).await.unwrap();
    assert!(listed.usdc_price(150).is_some_and(|price| price.is_positive()));
    same_shape(&client, "spot/spot_meta_and_asset_ctxs_mainnet.json");
    let details = spot::token_details(&client, &hype.base.token_id).await.unwrap();
    assert_eq!((details.name.as_str(), details.sz_decimals), ("HYPE", hype.base.sz_decimals));
    same_shape(&client, "spot/token_details_hype_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn outcomes_list_their_questions_templates_and_what_a_venue_may_still_list() {
    let client = reader();
    let meta = outcomes::outcome_meta(&client).await.unwrap();
    assert!(!meta.outcomes.is_empty() && !meta.deployers.is_empty() && meta.fee_scale.is_some());
    same_shape(&client, "hip4/outcome_meta_mainnet.json");
    let templates = outcomes::outcome_templates(&client).await.unwrap();
    assert!(templates.iter().any(|template| template.id == "binaryPrice"));
    same_shape(&client, "hip4/outcome_templates_mainnet.json");
    assert!(meta.deployer("out").is_some());
    outcomes::deployer_limits(&client, "out").await.unwrap();
    same_shape(&client, "hip4/outcome_deployer_limits_out_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn an_outcome_side_s_book_reads_on_the_grid_and_a_settled_one_names_its_payout() {
    let client = reader();
    let meta = outcomes::outcome_meta(&client).await.unwrap();
    let side = meta.outcomes.first().map(|outcome| outcome.coin(OutcomeSide::Yes)).unwrap();
    let read = markets::book::l2_book(&client, &side.to_string(), Precision::FULL).await.unwrap();
    same_shape(&client, "hip4/l2_book_14730_mainnet.json");
    assert_eq!(OutcomeBook::try_from(read).map(|book| book.coin), Ok(side));
    let settled = outcomes::settled_outcome(&client, 6290).await.unwrap().unwrap();
    assert_eq!(settled.winner(), Some(OutcomeSide::No));
    same_shape(&client, "hip4/settled_outcome_6290_mainnet.json");
}
