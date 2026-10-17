use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::TransportError;
use crate::trade::{MarginMode, Side, Tif};
use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::*;
use crate::trade::perps::{LeverageError, add_margin, remove_margin, update_leverage};

/// Live, mainnet, 2026-10-06, trimmed: the first three perps and their contexts as sent, then HPOS
/// (delisted, strictly isolated; 33rd live) and its context.
const META: &str = r#"{"universe":[{"szDecimals":5,"name":"BTC","maxLeverage":40,"marginTableId":56},{"szDecimals":4,"name":"ETH","maxLeverage":25,"marginTableId":55},{"szDecimals":2,"name":"ATOM","maxLeverage":5,"marginTableId":5},{"szDecimals":0,"name":"HPOS","maxLeverage":3,"marginTableId":3,"onlyIsolated":true,"isDelisted":true,"marginMode":"strictIsolated"}],"marginTables":[[50,{"description":"","marginTiers":[{"lowerBound":"0.0","maxLeverage":50}]}]],"collateralToken":0}"#;
const CTXS: &str = r#"[{"funding":"0.0000125","openInterest":"40762.72024","prevDayPx":"85286.0","dayNtlVlm":"1825095986.3497385979","premium":"-0.0002918515","oraclePx":"85660.0","markPx":"85634.0","midPx":"85634.5","impactPxs":["85631.1","85635.0"],"dayBaseVlm":"21250.29166"},{"funding":"0.0000125","openInterest":"1178184.1025999999","prevDayPx":"2695.6","dayNtlVlm":"665475998.0603500605","premium":"-0.0003446832","oraclePx":"2698.13","markPx":"2697.1","midPx":"2697.15","impactPxs":["2697.1","2697.2"],"dayBaseVlm":"245640.2553999999"},{"funding":"0.0000007923","openInterest":"1556009.5","prevDayPx":"1.796","dayNtlVlm":"690086.4431889998","premium":"-0.0002230276","oraclePx":"1.7935","markPx":"1.7923","midPx":"1.7923","impactPxs":["1.7915","1.7931"],"dayBaseVlm":"382125.4999999999"},{"funding":"0.0","openInterest":"0.0","prevDayPx":"0.041845","dayNtlVlm":"0.0","premium":null,"oraclePx":"0.042203","markPx":"0.041873","midPx":null,"impactPxs":null,"dayBaseVlm":"0.0"}]"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn both() -> MetaAndAssetCtxs {
    serde_json::from_str(&format!("[{META},{CTXS}]")).unwrap()
}

#[test]
fn meta_reads_the_live_answer_and_names_each_perp_s_asset_and_grid() {
    let meta: Meta = serde_json::from_str(META).unwrap();
    assert_eq!(meta.universe.len(), 4);
    assert_eq!(meta.collateral_token, Some(0));
    let (asset, btc) = meta.find("BTC").unwrap();
    assert_eq!((asset.id(), btc.sz_decimals, btc.max_leverage), (0, 5, 40));
    assert_eq!(btc.grid(), Grid::perp(5));
    assert_eq!(meta.find("ATOM").unwrap().0, Asset::perp(2).unwrap());
    assert_eq!(meta.find("btc"), None, "names are matched as the venue spells them");
    assert_eq!(meta.get(Asset::perp(1).unwrap()).map(|perp| perp.name.as_str()), Some("ETH"));
    assert_eq!(meta.get(Asset::from_id(10_001)), None, "a spot id is not a perp");
    let hpos = meta.find("HPOS").unwrap().1;
    assert!(hpos.is_delisted && hpos.only_isolated && !hpos.allows_cross());
    assert!(btc.allows_cross());
}

#[test]
fn contexts_line_up_with_the_universe_and_a_quiet_market_has_no_mid() {
    let both = both();
    let (asset, _, btc) = both.find("BTC").unwrap();
    assert_eq!(asset.id(), 0);
    assert_eq!((btc.mark_px, btc.mid_px, btc.reference()), (dec("85634"), Some(dec("85634.5")), dec("85634.5")));
    assert_eq!(btc.funding, dec("0.0000125"));
    assert_eq!(btc.impact_pxs, Some(vec![dec("85631.1"), dec("85635")]));
    let (_, _, hpos) = both.find("HPOS").unwrap();
    assert_eq!((hpos.mid_px, hpos.premium, hpos.reference()), (None, None, dec("0.041873")));
    assert_eq!(both.meta.margin_tables.len(), 1, "the one table this trimmed answer kept");
    assert_eq!(both.meta.margin_table(50).and_then(|table| table.max_leverage_at(Dec::ZERO)), Some(50));
}

/// BTC's context recorded live on mainnet, 2026-09-26, when its day change was -0.68%.
#[test]
fn a_context_says_its_day_change_and_its_open_interest_at_the_mark() {
    let btc: PerpCtx = serde_json::from_str(r#"{"funding":"0.0000125","openInterest":"37697.76106","prevDayPx":"84556.0","dayNtlVlm":"2460964638.0624189377","premium":"-0.0004701178","oraclePx":"84021.5","markPx":"83982.0","midPx":"83981.5","impactPxs":["83981.0","83982.0"],"dayBaseVlm":"29257.91492"}"#).unwrap();
    assert_eq!(btc.day_change(), Some(dec("-0.68")));
    assert_eq!(btc.open_interest_value(), Some(dec("3165933369.34092")));
    let both = both();
    assert_eq!(both.find("BTC").unwrap().2.day_change(), Some(dec("0.41")));
}

#[test]
fn a_market_order_goes_out_on_the_perp_s_grid_as_frontend_market() {
    let both = both();
    let (asset, btc, ctx) = both.find("BTC").unwrap();
    let order = btc.grid().market(asset, Side::Buy, ctx.reference(), dec("0.01"), dec("0.001234")).unwrap();
    assert_eq!((order.price(), order.size(), order.tif()), (dec("86490"), dec("0.00123"), Some(Tif::FrontendMarket)));
}

#[test]
fn leverage_is_held_to_what_the_asset_allows() {
    let meta: Meta = serde_json::from_str(META).unwrap();
    let (btc_asset, btc) = meta.find("BTC").unwrap();
    let set = update_leverage(btc_asset, btc, MarginMode::Cross, 40).unwrap();
    assert_eq!(set.wire(), r#"{"type":"updateLeverage","asset":0,"isCross":true,"leverage":40}"#);
    assert_eq!(
        update_leverage(btc_asset, btc, MarginMode::Cross, 41),
        Err(LeverageError::TooHigh { name: "BTC".to_owned(), max: 40, asked: 41 })
    );
    let (hpos_asset, hpos) = meta.find("HPOS").unwrap();
    assert_eq!(update_leverage(hpos_asset, hpos, MarginMode::Cross, 2), Err(LeverageError::NoCross("HPOS".to_owned())));
    assert!(update_leverage(hpos_asset, hpos, MarginMode::Isolated, 2).is_ok());
    assert!(matches!(update_leverage(btc_asset, btc, MarginMode::Isolated, 0), Err(LeverageError::Trade(_))));
}

#[test]
fn margin_goes_in_positive_and_comes_out_negative() {
    let eth = Asset::perp(1).unwrap();
    assert_eq!(add_margin(eth, Side::Buy, dec("12.5")).unwrap().ntli(), 12_500_000);
    assert_eq!(remove_margin(eth, Side::Sell, dec("2.5")).unwrap().ntli(), -2_500_000);
    assert!(add_margin(eth, Side::Buy, dec("-1")).is_err(), "a negative add is a removal, said as one");
    assert!(remove_margin(eth, Side::Buy, Dec::ZERO).is_err());
}

struct Recorded {
    answer: String,
    sent: RefCell<Vec<(String, String)>>,
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push((path.to_owned(), body));
        ready(Ok(self.answer.clone()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn the_reads_post_their_type_to_info() {
    let transport = Recorded { answer: format!("[{META},{CTXS}]"), sent: RefCell::new(Vec::new()) };
    let both = block_on(meta_and_asset_ctxs(&transport)).unwrap();
    assert_eq!(both.ctxs.len(), 4);
    assert_eq!(transport.sent.borrow()[0], ("/info".to_owned(), r#"{"type":"metaAndAssetCtxs"}"#.to_owned()));
    assert_eq!(meta_request(), r#"{"type":"meta"}"#);
}
