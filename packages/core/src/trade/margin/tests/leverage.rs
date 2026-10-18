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
fn a_ticket_s_leverage_is_held_to_the_market_s_ceiling_and_margin_mode() {
    assert_eq!(
        wire(leverage(&tsla(), MarginMode::Isolated, 10)),
        r#"{"type":"updateLeverage","asset":110001,"isCross":false,"leverage":10}"#
    );
    for (market, mode, asked, refused) in [
        (tsla(), MarginMode::Cross, 10, Refusal::IsolatedOnly),
        (tsla(), MarginMode::Isolated, 11, Refusal::Leverage { asked: 11, most: 10 }),
        (tsla(), MarginMode::Isolated, 0, Refusal::Leverage { asked: 0, most: 10 }),
        (hype(), MarginMode::Cross, 2, Refusal::Spot("leverage")),
    ] {
        assert_eq!(leverage(&market, mode, asked), Err(refused), "{asked}x");
    }
    assert_eq!(
        leverage(&tsla(), MarginMode::Isolated, 11).unwrap_err().to_string(),
        "leverage is a whole number from 1 to 10, not 11"
    );
}

/// The main dex's BTC and HPOS, and HIP-3's `xyz:TSLA` and `para:AVGO`, as the venue lists them.
#[test]
fn leverage_is_held_to_what_each_perp_and_the_tier_its_position_reaches_allow() {
    let meta: Meta = serde_json::from_str(MAINNET).unwrap();
    let dexes = Dexes::new(serde_json::from_str(DEXES).unwrap(), serde_json::from_str(METAS).unwrap()).unwrap();
    let (btc_at, btc) = meta.find(0, "BTC").unwrap();
    let (hpos_at, hpos) = meta.find(0, "HPOS").unwrap();
    let (tsla_at, tsla) = dexes.find("xyz:TSLA").unwrap();
    let (avgo_at, avgo) = dexes.find("para:AVGO").unwrap();
    let too_high = |name: &str, max: u32, asked: u32| Err(LeverageError::TooHigh { name: name.to_owned(), max, asked });
    for (at, perp, mode, asked, expected) in [
        (btc_at, btc, MarginMode::Cross, 40, Ok(40)),
        (btc_at, btc, MarginMode::Cross, 41, too_high("BTC", 40, 41)),
        (btc_at, btc, MarginMode::Isolated, 0, Err(LeverageError::Trade(TradeError::NoLeverage))),
        (hpos_at, hpos, MarginMode::Cross, 2, Err(LeverageError::NoCross("HPOS".to_owned()))),
        (hpos_at, hpos, MarginMode::Isolated, 2, Ok(2)),
        (tsla_at, tsla, MarginMode::Cross, 20, Ok(20)),
        (tsla_at, tsla, MarginMode::Cross, 21, too_high("xyz:TSLA", 20, 21)),
        (avgo_at, avgo, MarginMode::Cross, 2, Err(LeverageError::NoCross("para:AVGO".to_owned()))),
    ] {
        assert_eq!(
            update_leverage(at, perp, mode, asked).map(|set| set.leverage()),
            expected,
            "{} {asked}x",
            perp.name
        );
    }
    assert_eq!(
        update_leverage(btc_at, btc, MarginMode::Cross, 40).unwrap().wire(),
        r#"{"type":"updateLeverage","asset":0,"isCross":true,"leverage":40}"#
    );

    let btc_table = meta.table_of(btc).unwrap();
    let (_, _, para) = dexes.dex("para").unwrap();
    let avgo_table = para.table_of(avgo).unwrap();
    for (at, perp, table, mode, asked, notional, expected) in [
        (btc_at, btc, &btc_table, MarginMode::Cross, 40, million(1), Ok(40)),
        (
            btc_at,
            btc,
            &btc_table,
            MarginMode::Cross,
            25,
            million(200),
            Err(LeverageError::OverTier { name: "BTC".to_owned(), notional: million(200), max: 20, asked: 25 }),
        ),
        (btc_at, btc, &btc_table, MarginMode::Isolated, 20, million(200), Ok(20)),
        (btc_at, btc, &btc_table, MarginMode::Cross, 41, Dec::ZERO, too_high("BTC", 40, 41)),
        (avgo_at, avgo, &avgo_table, MarginMode::Isolated, 10, million(1), Ok(10)),
    ] {
        let set = update_leverage_for(at, perp, table, mode, asked, notional);
        assert_eq!(set.map(|set| set.leverage()), expected, "{} {asked}x at {notional}", perp.name);
    }
}
