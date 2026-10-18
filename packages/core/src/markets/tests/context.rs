//! Recorded mainnet contexts: BTC's `activeAssetCtx` on 2026-09-26 and 2026-10-08, and an outcome
//! side's `activeSpotAssetCtx` with no book on 2026-10-08.

use super::*;

const BTC: &str = r#"{"funding":"0.0000078567","openInterest":"39907.5661399998","prevDayPx":"83180.0","dayNtlVlm":"3316947851.6990442276","premium":"-0.0003826472","oraclePx":"80491.9","markPx":"80454.0","midPx":"80459.5","impactPxs":["80459.0","80461.1"],"dayBaseVlm":"40346.9258300001"}"#;
const BTC_EARLIER: &str = r#"{"funding":"0.0000125","openInterest":"37697.76106","prevDayPx":"84556.0","dayNtlVlm":"2460964638.0624189377","premium":"-0.0004701178","oraclePx":"84021.5","markPx":"83982.0","midPx":"83981.5","impactPxs":["83981.0","83982.0"],"dayBaseVlm":"29257.91492"}"#;
const OUTCOME: &str = r##"{"prevDayPx":"0.0","dayNtlVlm":"0.0","markPx":"0.5","midPx":null,"circulatingSupply":"34789.0","coin":"#14720","totalSupply":"184467440737095.53125","dayBaseVlm":"0.0"}"##;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_perp_is_bounded_from_its_mid_and_says_its_day_and_open_interest() {
    let btc: PerpCtx = serde_json::from_str(BTC).unwrap();
    assert_eq!((btc.reference(), btc.funding), (dec("80459.5"), dec("0.0000078567")));
    assert_eq!(btc.impact_pxs, Some(vec![dec("80459"), dec("80461.1")]));
    assert_eq!(btc.day_change(), Some(dec("-3.28")), "80454 against 83180");
    let earlier: PerpCtx = serde_json::from_str(BTC_EARLIER).unwrap();
    assert_eq!(earlier.day_change(), Some(dec("-0.68")), "to two places, the nearest");
    assert_eq!(earlier.open_interest_value(), Some(dec("3165933369.34092")));
    let quiet = PerpCtx { mid_px: None, prev_day_px: Dec::ZERO, ..btc };
    assert_eq!((quiet.reference(), quiet.day_change()), (dec("80454"), None), "the mark; no day without a price then");
}

#[test]
fn an_outcome_side_with_no_book_is_bounded_from_its_mark() {
    let side: SpotCtx = serde_json::from_str(OUTCOME).unwrap();
    assert_eq!((side.coin.as_str(), side.mid_px, side.reference()), ("#14720", None, dec("0.5")));
    assert_eq!((side.circulating_supply, side.total_supply), (dec("34789"), Some(dec("184467440737095.53125"))));
    let without = OUTCOME.replace(r#""circulatingSupply":"34789.0","#, "");
    assert!(serde_json::from_str::<SpotCtx>(&without).is_err(), "every context carries its circulating supply");
}
