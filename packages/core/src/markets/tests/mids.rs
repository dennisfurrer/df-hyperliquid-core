//! Mids read on mainnet, 2026-10-08, on the main dex and on `xyz`, a few coins of each kept. Reads
//! through a client are in `tests/mocked/markets`.

use super::*;

const MIDS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/all_mids.json"));
const MIDS_XYZ: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/all_mids_xyz.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn mids_read_by_coin_as_the_venue_spells_it_in_the_order_written() {
    let main: Mids = serde_json::from_str(MIDS).unwrap();
    let cases = [
        ("BTC", Some(dec("80847.5"))),
        ("@107", Some(dec("83.524"))),
        ("PURR/USDC", Some(dec("0.11205"))),
        ("#14730", Some(dec("0.612995"))),
        ("DOGE", None),
    ];
    for (coin, mid) in cases {
        assert_eq!(main.get(coin), mid, "{coin}");
    }
    let xyz: Mids = serde_json::from_str(MIDS_XYZ).unwrap();
    assert_eq!((xyz.get("xyz:AAPL"), xyz.get("AAPL")), (Some(dec("338.28")), None), "a HIP-3 coin carries its dex");
    let coins: Vec<&str> = xyz.iter().map(|(coin, _)| coin).collect();
    assert_eq!(coins, ["xyz:AAOI", "xyz:AAPL", "xyz:ACN"]);
    assert_eq!((main.len(), xyz.is_empty(), Mids::default().is_empty()), (6, false, true));
    let collected: Mids = xyz.iter().map(|(coin, mid)| (coin.to_owned(), mid)).collect();
    assert_eq!(collected, xyz);
    assert!(serde_json::from_str::<Mids>(r#"{"BTC": 1.5}"#).is_err(), "a float is refused, never rounded");
    assert!(serde_json::from_str::<Mids>("[]").is_err(), "a list is no map");
}
