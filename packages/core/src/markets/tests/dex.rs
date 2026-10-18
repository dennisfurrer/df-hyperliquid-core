//! `xyz` is dex 1, `flx` dex 2 and `para` dex 8 in `perpDexs`: `xyz:TSLA` is asset 110001 and
//! `flx`'s first perp 120000.

use super::*;
use crate::utils::asset::Asset;

#[test]
fn a_coin_names_its_dex_and_splits_into_dex_and_symbol() {
    let cases = [
        ("xyz:TSLA", Dex::Hip3("xyz"), Some(("xyz", "TSLA"))),
        ("para:10Y", Dex::Hip3("para"), Some(("para", "10Y"))),
        ("BTC", Dex::Main, None),
        (":TSLA", Dex::Hip3(""), None),
        ("xyz:", Dex::Hip3("xyz"), None),
    ];
    for (text, dex, parts) in cases {
        assert_eq!((Dex::of_coin(text), split_coin(text)), (dex, parts), "{text}");
    }
    assert_eq!(coin("flx", "GOLD"), "flx:GOLD");
    assert_eq!(Dex::default(), Dex::Main);
    assert_eq!((Dex::Main.name(), Dex::Hip3("flx").name()), (None, Some("flx")));
    assert_eq!((Dex::Main.field(), Dex::Hip3("flx").field()), (None, Some(("dex", Json::String("flx".to_owned())))));
}

#[test]
fn only_a_hip3_asset_id_gives_back_its_dex_and_index() {
    let cases = [
        (110_001, Some((1, 1))),
        (120_000, Some((2, 0))),
        (180_015, Some((8, 15))),
        (219_999, Some((11, 9_999))),
        (0, None),
        (10_107, None),
        (100_000, None),
        (100_012_100, None),
    ];
    for (id, parts) in cases {
        assert_eq!(Asset::from_id(id).hip3_parts(), parts, "{id}");
    }
    for (dex, index) in [(1, 0), (8, 15), (9_989, 9_999)] {
        assert_eq!(Asset::hip3(dex, index).and_then(Asset::hip3_parts), Some((dex, index)));
    }
}
