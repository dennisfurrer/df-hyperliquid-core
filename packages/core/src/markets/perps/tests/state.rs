//! Recorded mainnet answers. `clearinghouse_hlp_a_mainnet.json`: HLP Strategy A
//! (`0x010461c1..703a`), 2026-10-08, 177 positions cut to four; the summaries are as sent, so they
//! count the positions left out. `clearinghouse_isolated_mainnet.json` and
//! `clearinghouse_xyz_empty_mainnet.json`: the main dex and `xyz` from one
//! `allDexsClearinghouseState` frame, 2026-09-26. `active_asset_data_btc_mainnet.json`: the same
//! vault on BTC, a minute after its clearinghouse.

use crate::account::active::{ActiveAssetData, active_asset_data, active_asset_data_request};
use crate::account::positions::{ClearinghouseState, Leverage, clearinghouse_state, clearinghouse_state_request};
use crate::markets::Dex;
use crate::markets::perps::support::{Recorded, block_on, dec};
use crate::trade::{MarginMode, Side};
use crate::utils::{Address, Dec};

const HLP_A: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_hlp_a_mainnet.json"));
const ISOLATED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_isolated_mainnet.json"));
const XYZ_EMPTY: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/clearinghouse_xyz_empty_mainnet.json"));
const ACTIVE_BTC: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/active_asset_data_btc_mainnet.json"));

fn vault() -> Address {
    "0x010461C14e146ac35Fe42271BDC1134EE31C703a".parse().unwrap()
}

fn state(text: &str) -> ClearinghouseState {
    serde_json::from_str(text).unwrap()
}

#[test]
fn a_cross_position_reads_every_figure_as_sent() {
    let state = state(HLP_A);
    let btc = state.position("BTC").unwrap();
    assert_eq!(btc.szi, dec("0.75104"));
    assert_eq!(btc.entry_px, Some(dec("82511.5")));
    assert_eq!(btc.position_value, dec("60635.2144"));
    assert_eq!(btc.unrealized_pnl, dec("-1334.284132"));
    assert_eq!(btc.return_on_equity, Some(dec("-0.4306260866")));
    assert_eq!(btc.liquidation_px, None, "a vault this well margined has none");
    assert_eq!(btc.margin_used, dec("3031.76072"));
    assert_eq!(btc.leverage, Leverage::Cross { value: 20 });
    assert_eq!((btc.leverage.mode(), btc.leverage.value()), (MarginMode::Cross, 20));
    assert_eq!(btc.max_leverage, 50);
    let funding = btc.cum_funding.as_ref().unwrap();
    assert_eq!(
        (funding.all_time, funding.since_open, funding.since_change),
        (dec("-612507.411689"), dec("490.399487"), Dec::ZERO)
    );
    assert!(btc.is_long() && !btc.is_short());
    assert_eq!(btc.size(), dec("0.75104"));
    assert_eq!(btc.closing_side(), Some(Side::Sell));
}

#[test]
fn a_short_reads_negative_and_closes_with_a_buy() {
    let state = state(HLP_A);
    let dydx = state.position("DYDX").unwrap();
    assert_eq!(dydx.szi, dec("-303109.2"));
    assert!(dydx.is_short());
    assert_eq!(dydx.size(), dec("303109.2"));
    assert_eq!(dydx.closing_side(), Some(Side::Buy));
    assert_eq!(dydx.liquidation_px, Some(dec("10.3702027403")));
}

/// The venue's position value is the size at the mark, so the mark it implies lands on the price
/// grid: 80735 is a whole BTC price, 0.12713 a five-figure DYDX one.
#[test]
fn the_mark_a_position_implies_is_its_value_over_its_size() {
    let state = state(HLP_A);
    let marks: Vec<(&str, Dec)> =
        state.positions().map(|position| (position.coin.as_str(), position.mark().unwrap())).collect();
    assert_eq!(
        marks,
        [("BTC", dec("80735")), ("ETH", dec("2419.8")), ("DYDX", dec("0.12713")), ("AVAX", dec("9.8715"))]
    );
}

#[test]
fn the_margin_summaries_and_withdrawable_read_as_sent() {
    let state = state(HLP_A);
    assert_eq!(state.margin_summary.account_value, dec("3173167.3515860001"));
    assert_eq!(state.margin_summary.total_ntl_pos, dec("3735007.1955929999"));
    assert_eq!(state.margin_summary.total_raw_usd, dec("2920252.4349730001"));
    assert_eq!(state.margin_summary.total_margin_used, dec("186750.359729"));
    assert_eq!(state.cross_margin_summary, state.margin_summary, "all of it cross");
    assert_eq!(state.cross_maintenance_margin_used, dec("37350.071885"));
    assert_eq!(state.withdrawable, dec("2799666.6320639998"));
    assert_eq!(state.time, Some(1_791_480_567_359));
}

/// An isolated position carries `rawUsd`, and what it uses is its value plus that, to the last
/// digit. The account's value is likewise its raw USD plus its notional.
#[test]
fn an_isolated_position_carries_its_raw_usd() {
    let state = state(ISOLATED);
    let btc = state.position("BTC").unwrap();
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    assert_eq!(btc.leverage.mode(), MarginMode::Isolated);
    assert_eq!(btc.position_value.checked_add(btc.leverage.raw_usd().unwrap()), Some(btc.margin_used));
    assert_eq!(btc.liquidation_px, Some(dec("63823.6021044955")));
    assert_eq!(btc.mark(), Some(dec("83974")));
    let summary = &state.margin_summary;
    assert_eq!(summary.total_raw_usd.checked_add(summary.total_ntl_pos), Some(summary.account_value));
    assert_eq!(state.cross_margin_summary.account_value, Dec::ZERO, "nothing is cross");
}

#[test]
fn a_dex_with_nothing_on_it_has_no_positions() {
    let state = state(XYZ_EMPTY);
    assert_eq!(state.positions().count(), 0);
    assert!(!state.holds_anything());
    assert!(serde_json::from_str::<ClearinghouseState>(ISOLATED).unwrap().holds_anything());
}

/// A recorded subaccount state with a cross short beside a flat leg: no `cumFunding`, and `entryPx`
/// and `liquidationPx` null on the flat one, which [`ClearinghouseState::positions`] leaves out.
#[test]
fn a_flat_leg_is_not_a_position() {
    let text = r#"{"marginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999","totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},"crossMarginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999","totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},"crossMaintenanceMarginUsed":"29231.7948","withdrawable":"1204.55","assetPositions":[{"type":"oneWay","position":{"coin":"HYPE","szi":"-6267.0","leverage":{"type":"cross","value":3},"entryPx":"13.1333","positionValue":"584635.8959999999","unrealizedPnl":"-502329.38087","returnOnEquity":"-18.3094635975","liquidationPx":"132.9707433478","marginUsed":"194878.632","maxLeverage":10}},{"type":"oneWay","position":{"coin":"BTC","szi":"0.0","leverage":{"type":"cross","value":5},"entryPx":null,"positionValue":"0.0","unrealizedPnl":"0.0","returnOnEquity":"0.0","liquidationPx":null,"marginUsed":"0.0","maxLeverage":40}}],"time":1790416405409}"#;
    let state: ClearinghouseState = serde_json::from_str(text).unwrap();
    assert_eq!(state.asset_positions.len(), 2);
    let held: Vec<&str> = state.positions().map(|position| position.coin.as_str()).collect();
    assert_eq!(held, ["HYPE"]);
    let btc = &state.asset_positions[1].position;
    assert_eq!((btc.entry_px, btc.cum_funding.is_none(), btc.mark(), btc.closing_side()), (None, true, None, None));
}

/// A subaccount's state carries no `time` and no `returnOnEquity` per position: it still reads,
/// and reports both as unknown, never 1970 or zero.
#[test]
fn a_state_without_its_time_or_return_on_equity_says_so() {
    let mut text: serde_json::Value = serde_json::from_str(XYZ_EMPTY).unwrap();
    text.as_object_mut().unwrap().remove("time");
    assert_eq!(serde_json::from_value::<ClearinghouseState>(text).unwrap().time, None);
    let mut text: serde_json::Value = serde_json::from_str(ISOLATED).unwrap();
    text["assetPositions"][0]["position"].as_object_mut().unwrap().remove("returnOnEquity");
    let state: ClearinghouseState = serde_json::from_value(text).unwrap();
    assert_eq!(state.asset_positions[0].position.return_on_equity, None);
}

#[test]
fn the_dex_is_named_only_for_a_hip3_dex() {
    let user = vault();
    assert_eq!(
        clearinghouse_state_request(user, Dex::Main),
        r#"{"type":"clearinghouseState","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a"}"#
    );
    assert_eq!(
        clearinghouse_state_request(user, Dex::Hip3("xyz")),
        r#"{"type":"clearinghouseState","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a","dex":"xyz"}"#
    );
    assert_eq!(Dex::of_coin("xyz:XYZ100"), Dex::Hip3("xyz"));
    assert_eq!(Dex::of_coin("BTC"), Dex::Main);
    assert_eq!((Dex::Main.name(), Dex::Hip3("flx").name()), (None, Some("flx")));
}

#[test]
fn the_reads_post_what_they_are_asked_and_read_the_answer() {
    let transport = Recorded::new(&[
        ("clearinghouseState", HLP_A),
        ("clearinghouseState", XYZ_EMPTY),
        ("activeAssetData", ACTIVE_BTC),
    ]);
    let main = block_on(clearinghouse_state(&transport, vault(), Dex::Main)).unwrap();
    assert_eq!(main.positions().count(), 4);
    let xyz = block_on(clearinghouse_state(&transport, vault(), Dex::Hip3("xyz"))).unwrap();
    assert!(!xyz.holds_anything());
    let active = block_on(active_asset_data(&transport, vault(), "BTC")).unwrap();
    assert_eq!(active.coin, "BTC");
    assert_eq!(
        transport.sent()[2],
        r#"{"type":"activeAssetData","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a","coin":"BTC"}"#
    );
    assert_eq!(
        active_asset_data_request(vault(), "xyz:XYZ100"),
        r#"{"type":"activeAssetData","user":"0x010461c14e146ac35fe42271bdc1134ee31c703a","coin":"xyz:XYZ100"}"#
    );
}

/// The leverage `activeAssetData` reports in force is the one the position in
/// `clearinghouseState` carries: two reads that agree.
#[test]
fn active_asset_data_says_the_leverage_in_force_and_what_each_side_may_trade() {
    let active: ActiveAssetData = serde_json::from_str(ACTIVE_BTC).unwrap();
    assert_eq!(active.user, vault());
    assert_eq!(active.leverage, state(HLP_A).position("BTC").unwrap().leverage);
    assert_eq!(active.mark_px, dec("80674"));
    assert_eq!(active.max_size(Side::Buy), dec("740.51701"));
    assert_eq!(active.max_size(Side::Sell), dec("742.01909"));
    assert_eq!(active.available(Side::Buy), dec("2987023.4632370002"));
    assert_eq!(active.available(Side::Sell), dec("2993082.4033329999"));
    // Long BTC, the vault may sell more than it may buy: a sell closes the long first, which frees
    // its margin.
    assert!(active.available(Side::Sell) > active.available(Side::Buy));
}
