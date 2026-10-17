//! `meta_mainnet.json`: the main dex's `meta`, live, mainnet, 2026-09-27 (BTC 40x up to 150M of
//! notional and 20x past it). `perp_dexs_mainnet.json` and `all_perp_metas_mainnet.json`: every dex
//! and its meta, live, mainnet, 2026-09-26. HPOS is delisted and strictly isolated.

use super::*;
use crate::markets::dexes::Dexes;
use crate::markets::perps::Meta;
use crate::trade::margin::leverage;
use crate::trade::margin::tables::LeverageError;
use crate::trade::ticket_support::*;
use crate::utils::Dec;

const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));
const DEXES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_dexs_mainnet.json"));
const METAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/all_perp_metas_mainnet.json"));

fn million(n: i64) -> Dec {
    Dec::from_int(n * 1_000_000)
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
fn leverage_is_held_to_what_the_asset_allows() {
    let meta: Meta = serde_json::from_str(MAINNET).unwrap();
    let (btc_asset, btc) = meta.find(0, "BTC").unwrap();
    let set = update_leverage(btc_asset, btc, MarginMode::Cross, 40).unwrap();
    assert_eq!(set.wire(), r#"{"type":"updateLeverage","asset":0,"isCross":true,"leverage":40}"#);
    assert_eq!(
        update_leverage(btc_asset, btc, MarginMode::Cross, 41),
        Err(LeverageError::TooHigh { name: "BTC".to_owned(), max: 40, asked: 41 })
    );
    let (hpos_asset, hpos) = meta.find(0, "HPOS").unwrap();
    assert_eq!(update_leverage(hpos_asset, hpos, MarginMode::Cross, 2), Err(LeverageError::NoCross("HPOS".to_owned())));
    assert!(update_leverage(hpos_asset, hpos, MarginMode::Isolated, 2).is_ok());
    assert!(matches!(update_leverage(btc_asset, btc, MarginMode::Isolated, 0), Err(LeverageError::Trade(_))));
}

#[test]
fn leverage_is_held_to_the_tier_the_position_reaches() {
    let meta: Meta = serde_json::from_str(MAINNET).unwrap();
    let (asset, btc) = meta.find(0, "BTC").unwrap();
    let table = meta.table_of(btc).unwrap();
    let set = update_leverage_for(asset, btc, &table, MarginMode::Cross, 40, million(1)).unwrap();
    assert_eq!(set.leverage(), 40);
    assert_eq!(
        update_leverage_for(asset, btc, &table, MarginMode::Cross, 25, million(200)),
        Err(LeverageError::OverTier { name: "BTC".to_owned(), notional: million(200), max: 20, asked: 25 })
    );
    assert!(update_leverage_for(asset, btc, &table, MarginMode::Isolated, 20, million(200)).is_ok());
    assert_eq!(
        update_leverage_for(asset, btc, &table, MarginMode::Cross, 41, Dec::ZERO),
        Err(LeverageError::TooHigh { name: "BTC".to_owned(), max: 40, asked: 41 })
    );
}

#[test]
fn leverage_on_a_hip3_perp_is_held_to_what_it_allows() {
    let dexes = Dexes::new(serde_json::from_str(DEXES).unwrap(), serde_json::from_str(METAS).unwrap()).unwrap();
    let (asset, tsla) = dexes.find("xyz:TSLA").unwrap();
    let set = update_leverage(asset, tsla, MarginMode::Cross, 20).unwrap();
    assert_eq!(set.leverage(), 20);
    assert_eq!(
        update_leverage(asset, tsla, MarginMode::Cross, 21),
        Err(LeverageError::TooHigh { name: "xyz:TSLA".to_owned(), max: 20, asked: 21 })
    );
    let (asset, avgo) = dexes.find("para:AVGO").unwrap();
    assert_eq!(update_leverage(asset, avgo, MarginMode::Cross, 2), Err(LeverageError::NoCross("para:AVGO".to_owned())));
    let (_, _, para) = dexes.dex("para").unwrap();
    let table = para.table_of(avgo).unwrap();
    assert!(update_leverage_for(asset, avgo, &table, MarginMode::Isolated, 10, dec("1000000")).is_ok());
}
