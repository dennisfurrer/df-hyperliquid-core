//! `meta_mainnet.json` is the main dex's `meta` from `allPerpMetas`, live, mainnet, 2026-09-27: all
//! 234 perps and seven margin tables. The tiers match Hyperliquid's docs (trading/margin-tiers): BTC
//! 40x up to 150M of notional and 20x past it, ETH 25x up to 100M and 15x past it.

use super::*;
use crate::utils::Dec;

const META: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));
/// `allPerpMetas`, live, mainnet, 2026-09-26: all 11 dexes, the main dex's first.
const METAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/all_perp_metas_mainnet.json"));

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
    let (_, btc) = meta.find(0, "BTC").unwrap();
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
    let (_, eth) = meta.find(0, "ETH").unwrap();
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
    for (_, perp) in meta.perps(0) {
        let table = meta.table_of(perp).unwrap();
        assert_eq!(table.max_leverage_at(Dec::ZERO), Some(perp.max_leverage), "{}", perp.name);
    }
    let (_, atom) = meta.find(0, "ATOM").unwrap();
    assert_eq!(meta.table_of(atom), Some(MarginTable::flat(5)));
    assert_eq!(meta.margin_table(20), Some(MarginTable::flat(20)));
    assert_eq!(meta.margin_table(57), None, "an id past 50 the meta does not list");
}

/// Every HIP-3 perp's table starts at its `maxLeverage` too: a flat id under 50 is that leverage,
/// a listed one has it as its first tier.
#[test]
fn every_hip3_perp_s_table_starts_at_its_max_leverage() {
    let metas: Vec<Meta> = serde_json::from_str(METAS).unwrap();
    let mut count = 0;
    for meta in &metas[1..] {
        for perp in &meta.universe {
            let table = meta.table_of(perp).unwrap();
            assert_eq!(table.max_leverage_at(Dec::ZERO), Some(perp.max_leverage), "{}", perp.name);
            count += 1;
        }
    }
    assert_eq!(count, 294);
}
