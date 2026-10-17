//! A recorded `allMids` answer for `xyz` (mainnet, 2026-10-08, three coins kept).

use super::*;

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
