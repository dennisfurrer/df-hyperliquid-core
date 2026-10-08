//! hl-ws's recorded `activeAssetCtx` (BTC) and `activeSpotAssetCtx` (an outcome side with no
//! book) contexts, mainnet, 2026-10-08.

use super::*;

const PERP: &str = r#"{"funding":"0.0000078567","openInterest":"39907.5661399998","prevDayPx":"83180.0","dayNtlVlm":"3316947851.6990442276","premium":"-0.0003826472","oraclePx":"80491.9","markPx":"80454.0","midPx":"80459.5","impactPxs":["80459.0","80461.1"],"dayBaseVlm":"40346.9258300001"}"#;
const OUTCOME: &str = r##"{"prevDayPx":"0.0","dayNtlVlm":"0.0","markPx":"0.5","midPx":null,"circulatingSupply":"34789.0","coin":"#14720","totalSupply":"184467440737095.53125","dayBaseVlm":"0.0"}"##;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_perp_is_bounded_from_its_mid_and_says_its_day() {
    let btc: PerpCtx = serde_json::from_str(PERP).unwrap();
    assert_eq!((btc.reference(), btc.impact_pxs.as_ref().map(Vec::len)), (dec("80459.5"), Some(2)));
    assert_eq!(btc.day_change(), Some(dec("-3.28")), "80454 against 83180");
}

#[test]
fn an_outcome_side_with_no_book_is_bounded_from_its_mark() {
    let side: SpotCtx = serde_json::from_str(OUTCOME).unwrap();
    assert_eq!((side.mid_px, side.reference(), side.circulating_supply), (None, dec("0.5"), dec("34789")));
}
