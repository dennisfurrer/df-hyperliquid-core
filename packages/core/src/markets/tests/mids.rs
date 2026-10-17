//! A recorded `allMids` answer for `xyz` (mainnet, 2026-10-08, three coins kept), and mids read
//! live the same day on the main dex and on `xyz` (a few coins of each kept).

use super::*;
use crate::trade::tests_support::{Recorded, block_on};

const MIDS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/all_mids.json"));
const MIDS_XYZ: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/all_mids_xyz.json"));

const XYZ: &str = r#"{"xyz:AAOI": "106.975", "xyz:AAPL": "338.28", "xyz:ACN": "198.9"}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn mids_read_by_coin_and_keep_the_order_written() {
    let mids: Mids = serde_json::from_str(XYZ).unwrap();
    assert_eq!((mids.len(), mids.get("xyz:AAPL"), mids.get("AAPL")), (3, Some(dec("338.28")), None));
    let coins: Vec<&str> = mids.iter().map(|(coin, _)| coin).collect();
    assert_eq!(coins, ["xyz:AAOI", "xyz:AAPL", "xyz:ACN"]);
    assert!(serde_json::from_str::<Mids>(r#"{"BTC": 1.5}"#).is_err(), "a float is refused, never rounded");
}

#[test]
fn mids_are_read_per_dex() {
    assert_eq!(all_mids_request(Dex::Main), r#"{"type":"allMids"}"#);
    assert_eq!(all_mids_request(Dex::Hip3("xyz")), r#"{"type":"allMids","dex":"xyz"}"#);
    let mids: Mids = serde_json::from_str(MIDS).unwrap();
    assert_eq!(mids.get("BTC"), Some(dec("80847.5")));
    assert_eq!(mids.get("@107"), Some(dec("83.524")));
    assert_eq!(mids.get("#14730"), Some(dec("0.612995")), "an outcome's mid, finer than its grid");
    assert_eq!(mids.get("DOGE"), None);
    let xyz: Mids = serde_json::from_str(MIDS_XYZ).unwrap();
    assert_eq!(xyz.get("xyz:AAPL"), Some(dec("338.28")));
    assert_eq!(xyz.len(), 3);
    let transport = Recorded::answering(MIDS);
    assert_eq!(block_on(all_mids(&transport, Dex::Main)).unwrap().len(), 6);
}
