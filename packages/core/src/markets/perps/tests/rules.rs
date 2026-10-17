//! `meta_mainnet.json` is the main dex's `meta` from `allPerpMetas`, live, mainnet, 2026-09-27: all
//! 234 perps and seven margin tables. The tiers match Hyperliquid's docs (trading/margin-tiers): BTC
//! 40x up to 150M of notional and 20x past it, ETH 25x up to 100M and 15x past it.

use crate::markets::perps::meta::Meta;
use crate::markets::perps::rules::{Rules, underlying};
use crate::markets::perps::support::dec;
use crate::markets::perps::table::{FLAT_TABLES_BELOW, MarginTable, MarginTier};
use crate::trade::MarginMode;
use crate::trade::perps::{LeverageError, update_leverage_for};
use crate::utils::Dec;
use crate::utils::{Grid, MINIMUM_ORDER_VALUE};

const META: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));

fn meta() -> Meta {
    serde_json::from_str(META).unwrap()
}

fn million(n: i64) -> Dec {
    Dec::from_int(n * 1_000_000)
}

#[test]
fn the_tables_read_as_the_docs_state_them() {
    let meta = meta();
    assert_eq!(meta.margin_tables.len(), 7);
    let (_, btc) = meta.find("BTC").unwrap();
    let table = meta.table_of(btc).unwrap();
    assert_eq!(table.description, "tiered 40x");
    assert_eq!(
        table.margin_tiers,
        [
            MarginTier { lower_bound: Dec::ZERO, max_leverage: 40 },
            MarginTier { lower_bound: million(150), max_leverage: 20 }
        ]
    );
    assert_eq!(table.max_leverage_at(million(1)), Some(40));
    assert_eq!(table.max_leverage_at(million(150).checked_sub(Dec::ONE).unwrap()), Some(40));
    assert_eq!(table.max_leverage_at(million(200)), Some(20));
    assert_eq!(table.max_leverage_at(million(-200)), Some(20), "a short's notional counts as its size");
    let (_, eth) = meta.find("ETH").unwrap();
    let table = meta.table_of(eth).unwrap();
    assert_eq!((table.max_leverage_at(million(99)), table.max_leverage_at(million(101))), (Some(25), Some(15)));
}

/// A table id under 50 is a single tier at that leverage, so no such table is listed: every perp
/// on one has that id as its `maxLeverage`. A listed table's first tier is the perp's
/// `maxLeverage` in turn.
#[test]
fn every_perp_s_table_starts_at_its_max_leverage() {
    let meta = meta();
    assert!(meta.margin_tables.iter().all(|(id, _)| *id >= FLAT_TABLES_BELOW));
    for (_, perp) in meta.perps() {
        let table = meta.table_of(perp).unwrap();
        assert_eq!(table.max_leverage_at(Dec::ZERO), Some(perp.max_leverage), "{}", perp.name);
    }
    let (_, atom) = meta.find("ATOM").unwrap();
    assert_eq!(meta.table_of(atom), Some(MarginTable::flat(5)));
    assert_eq!(meta.margin_table(20), Some(MarginTable::flat(20)));
    assert_eq!(meta.margin_table(57), None, "an id past 50 the meta does not list");
}

#[test]
fn leverage_is_held_to_the_tier_the_position_reaches() {
    let meta = meta();
    let (asset, btc) = meta.find("BTC").unwrap();
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

/// Five significant figures at most `6 - szDecimals` places, sizes on `szDecimals`, 10 of the quote
/// at least, isolated only where the perp refuses cross.
#[test]
fn a_perp_s_rules_follow_its_meta() {
    let meta = meta();
    let (_, btc) = meta.find("BTC").unwrap();
    let rules = Rules::of(btc).unwrap();
    assert_eq!(rules.grid, Grid::perp(5));
    assert_eq!(rules.grid.price_places(), 1);
    assert_eq!(rules.size_step, dec("0.00001"));
    assert_eq!(rules.min_value, MINIMUM_ORDER_VALUE);
    assert_eq!((rules.max_leverage, rules.isolated_only), (40, false));
    assert_eq!(rules.margin_modes(), [MarginMode::Cross, MarginMode::Isolated]);
    let (_, kpepe) = meta.find("kPEPE").unwrap();
    assert_eq!(Rules::of(kpepe).unwrap().size_step, Dec::ONE);
    let isolated = meta.perps().map(|(_, perp)| perp).find(|perp| !perp.allows_cross()).unwrap();
    let rules = Rules::of(isolated).unwrap();
    assert!(rules.isolated_only);
    assert_eq!(rules.margin_modes(), [MarginMode::Isolated]);
}

/// `kPEPE` is a thousand PEPE. Of the 234 perps, the seven that start with a lower case `k` are
/// thousands; `KAS` and `KAITO` are not.
#[test]
fn a_thousands_perp_names_its_underlying_and_multiplier() {
    let meta = meta();
    let thousands: Vec<&str> = meta
        .perps()
        .filter(|(_, perp)| underlying(&perp.name).1 == Dec::from_int(1000))
        .map(|(_, perp)| underlying(&perp.name).0)
        .collect();
    assert_eq!(thousands, ["PEPE", "SHIB", "BONK", "LUNC", "FLOKI", "DOGS", "NEIRO"]);
    assert_eq!(underlying("KAITO"), ("KAITO", Dec::ONE));
    assert_eq!(underlying("BTC"), ("BTC", Dec::ONE));
    assert_eq!(underlying("xyz:TSLA"), ("TSLA", Dec::ONE), "a HIP-3 coin's dex is not its underlying");
    assert_eq!(underlying("k"), ("k", Dec::ONE));
}
