//! Recorded mainnet answers. `clearinghouse_hlp_a_mainnet.json`: HLP Strategy A
//! (`0x010461c1..703a`), 2026-10-08, 177 positions cut to four; the summaries are as sent, so they
//! count the positions left out. `clearinghouse_isolated_mainnet.json` and
//! `clearinghouse_xyz_empty_mainnet.json`: the main dex and `xyz` from one
//! `allDexsClearinghouseState` frame, 2026-09-26. `active_asset_data_btc_mainnet.json`: the same
//! vault on BTC, a minute after its clearinghouse.

use crate::account::{
    ActiveAssetData, ClearinghouseState, Leverage, active_asset_data, active_asset_data_request, clearinghouse_state,
    clearinghouse_state_request,
};
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
/// A subaccount's state: a cross short beside a flat leg, no `cumFunding`, `entryPx` and
/// `liquidationPx` null on the flat one.
const FLAT_LEG: &str = r#"{"marginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999","totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},"crossMarginSummary":{"accountValue":"41008.519458","totalNtlPos":"584635.8959999999","totalRawUsd":"625644.415458","totalMarginUsed":"194878.632"},"crossMaintenanceMarginUsed":"29231.7948","withdrawable":"1204.55","assetPositions":[{"type":"oneWay","position":{"coin":"HYPE","szi":"-6267.0","leverage":{"type":"cross","value":3},"entryPx":"13.1333","positionValue":"584635.8959999999","unrealizedPnl":"-502329.38087","returnOnEquity":"-18.3094635975","liquidationPx":"132.9707433478","marginUsed":"194878.632","maxLeverage":10}},{"type":"oneWay","position":{"coin":"BTC","szi":"0.0","leverage":{"type":"cross","value":5},"entryPx":null,"positionValue":"0.0","unrealizedPnl":"0.0","returnOnEquity":"0.0","liquidationPx":null,"marginUsed":"0.0","maxLeverage":40}}],"time":1790416405409}"#;

fn vault() -> Address {
    "0x010461C14e146ac35Fe42271BDC1134EE31C703a".parse().unwrap()
}

fn state(text: &str) -> ClearinghouseState {
    serde_json::from_str(text).unwrap()
}

/// The venue's position value is the size at the mark, so the mark it implies lands on the price
/// grid: 80735 is a whole BTC price, 0.12713 a five-figure DYDX one.
#[test]
fn a_cross_state_reads_every_figure_and_each_position_its_side_and_mark() {
    let state = state(HLP_A);
    let summary = &state.margin_summary;
    assert_eq!(
        (summary.account_value, summary.total_ntl_pos, summary.total_raw_usd, summary.total_margin_used),
        (dec("3173167.3515860001"), dec("3735007.1955929999"), dec("2920252.4349730001"), dec("186750.359729"))
    );
    assert_eq!(state.cross_margin_summary, state.margin_summary, "all of it cross");
    assert_eq!(
        (state.cross_maintenance_margin_used, state.withdrawable),
        (dec("37350.071885"), dec("2799666.6320639998"))
    );
    assert_eq!(state.time, Some(1_791_480_567_359));
    let btc = state.position("BTC").unwrap();
    assert_eq!((btc.szi, btc.entry_px, btc.position_value), (dec("0.75104"), Some(dec("82511.5")), dec("60635.2144")));
    assert_eq!((btc.unrealized_pnl, btc.return_on_equity), (dec("-1334.284132"), Some(dec("-0.4306260866"))));
    assert_eq!((btc.liquidation_px, btc.margin_used, btc.max_leverage), (None, dec("3031.76072"), 50));
    assert_eq!(
        (btc.leverage.clone(), btc.leverage.mode(), btc.leverage.value()),
        (Leverage::Cross { value: 20 }, MarginMode::Cross, 20)
    );
    let funding = btc.cum_funding.as_ref().unwrap();
    assert_eq!(
        (funding.all_time, funding.since_open, funding.since_change),
        (dec("-612507.411689"), dec("490.399487"), Dec::ZERO)
    );
    let dydx = state.position("DYDX").unwrap();
    assert_eq!((dydx.liquidation_px, dydx.size()), (Some(dec("10.3702027403")), dec("303109.2")));
    let sides = [("BTC", true, Side::Sell), ("DYDX", false, Side::Buy)];
    for (coin, long, closing) in sides {
        let position = state.position(coin).unwrap();
        assert_eq!(
            (position.is_long(), position.is_short(), position.closing_side()),
            (long, !long, Some(closing)),
            "{coin}"
        );
    }
    let marks: Vec<(&str, Dec)> =
        state.positions().map(|position| (position.coin.as_str(), position.mark().unwrap())).collect();
    assert_eq!(
        marks,
        [("BTC", dec("80735")), ("ETH", dec("2419.8")), ("DYDX", dec("0.12713")), ("AVAX", dec("9.8715"))]
    );
}

/// What an isolated position uses is its value plus its `rawUsd`, to the last digit; the account's
/// value is likewise its raw USD plus its notional.
#[test]
fn an_isolated_position_carries_its_raw_usd() {
    let state = state(ISOLATED);
    let btc = state.position("BTC").unwrap();
    assert_eq!(btc.leverage, Leverage::Isolated { value: 4, raw_usd: dec("-3828.81778") });
    assert_eq!(btc.leverage.mode(), MarginMode::Isolated);
    assert_eq!(btc.position_value.checked_add(btc.leverage.raw_usd().unwrap()), Some(btc.margin_used));
    assert_eq!((btc.liquidation_px, btc.mark()), (Some(dec("63823.6021044955")), Some(dec("83974"))));
    let summary = &state.margin_summary;
    assert_eq!(summary.total_raw_usd.checked_add(summary.total_ntl_pos), Some(summary.account_value));
    assert_eq!(state.cross_margin_summary.account_value, Dec::ZERO, "nothing is cross");
}

/// A flat leg, an empty dex and a subaccount's missing `time` and `returnOnEquity` read as nothing
/// held and unknown, never as a position, 1970 or zero.
#[test]
fn what_is_not_held_or_not_sent_reads_as_nothing() {
    let flat = state(FLAT_LEG);
    assert_eq!(flat.asset_positions.len(), 2);
    let held: Vec<&str> = flat.positions().map(|position| position.coin.as_str()).collect();
    assert_eq!(held, ["HYPE"]);
    let btc = &flat.asset_positions[1].position;
    assert_eq!((btc.entry_px, btc.cum_funding.is_none(), btc.mark(), btc.closing_side()), (None, true, None, None));
    let empty = state(XYZ_EMPTY);
    assert_eq!((empty.positions().count(), empty.holds_anything(), state(ISOLATED).holds_anything()), (0, false, true));
    let mut text: serde_json::Value = serde_json::from_str(XYZ_EMPTY).unwrap();
    text.as_object_mut().unwrap().remove("time");
    assert_eq!(serde_json::from_value::<ClearinghouseState>(text).unwrap().time, None);
    let mut text: serde_json::Value = serde_json::from_str(ISOLATED).unwrap();
    text["assetPositions"][0]["position"].as_object_mut().unwrap().remove("returnOnEquity");
    let state: ClearinghouseState = serde_json::from_value(text).unwrap();
    assert_eq!(state.asset_positions[0].position.return_on_equity, None);
}

/// The leverage `activeAssetData` reports in force is the one the position in
/// `clearinghouseState` carries: two reads that agree. Long BTC, the vault may sell more than it
/// may buy: a sell closes the long first, which frees its margin.
#[test]
fn active_asset_data_says_the_leverage_in_force_and_what_each_side_may_trade() {
    let active: ActiveAssetData = serde_json::from_str(ACTIVE_BTC).unwrap();
    assert_eq!((active.user, active.coin.as_str(), active.mark_px), (vault(), "BTC", dec("80674")));
    assert_eq!(active.leverage, state(HLP_A).position("BTC").unwrap().leverage);
    assert_eq!((active.max_size(Side::Buy), active.max_size(Side::Sell)), (dec("740.51701"), dec("742.01909")));
    assert_eq!(
        (active.available(Side::Buy), active.available(Side::Sell)),
        (dec("2987023.4632370002"), dec("2993082.4033329999"))
    );
    assert!(active.available(Side::Sell) > active.available(Side::Buy));
}

#[test]
fn the_account_reads_post_their_user_and_name_only_a_hip3_dex() {
    let user = r#""user":"0x010461c14e146ac35fe42271bdc1134ee31c703a""#;
    assert_eq!(clearinghouse_state_request(vault(), Dex::Main), format!(r#"{{"type":"clearinghouseState",{user}}}"#));
    assert_eq!(
        clearinghouse_state_request(vault(), Dex::Hip3("xyz")),
        format!(r#"{{"type":"clearinghouseState",{user},"dex":"xyz"}}"#)
    );
    assert_eq!(
        active_asset_data_request(vault(), "xyz:XYZ100"),
        format!(r#"{{"type":"activeAssetData",{user},"coin":"xyz:XYZ100"}}"#)
    );
    let transport = Recorded::new(&[
        ("clearinghouseState", HLP_A),
        ("clearinghouseState", XYZ_EMPTY),
        ("activeAssetData", ACTIVE_BTC),
    ]);
    assert_eq!(block_on(clearinghouse_state(&transport, vault(), Dex::Main)).unwrap().positions().count(), 4);
    assert!(!block_on(clearinghouse_state(&transport, vault(), Dex::Hip3("xyz"))).unwrap().holds_anything());
    assert_eq!(block_on(active_asset_data(&transport, vault(), "BTC")).unwrap().coin, "BTC");
    assert_eq!(transport.sent()[2], format!(r#"{{"type":"activeAssetData",{user},"coin":"BTC"}}"#));
}
