//! hl-perps' and hl-hip3's coins, and hip4-tui's reader: `xyz:TSLA` is
//! asset 110001 and `flx`'s first perp 120000.

use super::*;
use crate::asset::Asset;

#[test]
fn a_coin_names_its_dex() {
    assert_eq!(Dex::of_coin("xyz:XYZ100"), Dex::Hip3("xyz"));
    assert_eq!(Dex::of_coin("BTC"), Dex::Main);
    assert_eq!(Dex::default(), Dex::Main);
    assert_eq!((Dex::Main.name(), Dex::Hip3("flx").name()), (None, Some("flx")));
}

#[test]
fn a_hip3_coin_splits_into_dex_and_symbol() {
    assert_eq!(split_coin("xyz:TSLA"), Some(("xyz", "TSLA")));
    assert_eq!(split_coin("para:10Y"), Some(("para", "10Y")));
    assert_eq!(split_coin("BTC"), None, "a main-dex coin has no dex");
    assert_eq!(split_coin(":TSLA"), None);
    assert_eq!(split_coin("xyz:"), None);
    assert_eq!(coin("xyz", "TSLA"), "xyz:TSLA");
    assert_eq!(split_coin(&coin("flx", "GOLD")), Some(("flx", "GOLD")));
}

#[test]
fn a_hip3_asset_id_gives_back_its_dex_and_index() {
    assert_eq!(Asset::from_id(110_001).hip3_parts(), Some((1, 1)));
    assert_eq!(Asset::from_id(120_000).hip3_parts(), Some((2, 0)));
    assert_eq!(Asset::from_id(219_999).hip3_parts(), Some((11, 9999)));
    for (dex, index) in [(1, 0), (8, 15), (9_989, 9_999)] {
        assert_eq!(Asset::hip3(dex, index).and_then(Asset::hip3_parts), Some((dex, index)));
    }
}

#[test]
fn any_other_id_has_no_hip3_parts() {
    assert_eq!(Asset::from_id(0).hip3_parts(), None, "a main-dex perp");
    assert_eq!(Asset::from_id(10_107).hip3_parts(), None, "a spot pair");
    assert_eq!(Asset::from_id(100_000).hip3_parts(), None, "place 0 is the main dex");
    assert_eq!(Asset::from_id(100_012_100).hip3_parts(), None, "an outcome");
}
