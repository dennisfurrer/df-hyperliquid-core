use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::TransportError;
use crate::trade::{MarginMode, PlaceOrder, Side, Tif, UpdateLeverage};
use crate::utils::sign::L1Action;

use super::*;
use crate::trade::margin::{add_margin, remove_margin};

/// Live, mainnet, 2026-10-06, trimmed: the first three perps and their contexts as sent, then HPOS
/// (delisted, strictly isolated; 33rd live) and its context.
const META: &str = r#"{"universe":[{"szDecimals":5,"name":"BTC","maxLeverage":40,"marginTableId":56},{"szDecimals":4,"name":"ETH","maxLeverage":25,"marginTableId":55},{"szDecimals":2,"name":"ATOM","maxLeverage":5,"marginTableId":5},{"szDecimals":0,"name":"HPOS","maxLeverage":3,"marginTableId":3,"onlyIsolated":true,"isDelisted":true,"marginMode":"strictIsolated"}],"marginTables":[[50,{"description":"","marginTiers":[{"lowerBound":"0.0","maxLeverage":50}]}]],"collateralToken":0}"#;
const CTXS: &str = r#"[{"funding":"0.0000125","openInterest":"40762.72024","prevDayPx":"85286.0","dayNtlVlm":"1825095986.3497385979","premium":"-0.0002918515","oraclePx":"85660.0","markPx":"85634.0","midPx":"85634.5","impactPxs":["85631.1","85635.0"],"dayBaseVlm":"21250.29166"},{"funding":"0.0000125","openInterest":"1178184.1025999999","prevDayPx":"2695.6","dayNtlVlm":"665475998.0603500605","premium":"-0.0003446832","oraclePx":"2698.13","markPx":"2697.1","midPx":"2697.15","impactPxs":["2697.1","2697.2"],"dayBaseVlm":"245640.2553999999"},{"funding":"0.0000007923","openInterest":"1556009.5","prevDayPx":"1.796","dayNtlVlm":"690086.4431889998","premium":"-0.0002230276","oraclePx":"1.7935","markPx":"1.7923","midPx":"1.7923","impactPxs":["1.7915","1.7931"],"dayBaseVlm":"382125.4999999999"},{"funding":"0.0","openInterest":"0.0","prevDayPx":"0.041845","dayNtlVlm":"0.0","premium":null,"oraclePx":"0.042203","markPx":"0.041873","midPx":null,"impactPxs":null,"dayBaseVlm":"0.0"}]"#;

/// `meta` with `dex: "xyz"` and its contexts, the same day: the first two perps, HOOD (fifth live,
/// no cross) and URANIUM (delisted, no book).
const XYZ: &str = r#"{"universe":[{"szDecimals":4,"name":"xyz:XYZ100","maxLeverage":30,"marginTableId":30,"growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:TSLA","maxLeverage":20,"marginTableId":20,"growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:HOOD","maxLeverage":10,"marginTableId":10,"onlyIsolated":true,"marginMode":"noCross","growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:URANIUM","maxLeverage":10,"marginTableId":10,"onlyIsolated":true,"isDelisted":true,"marginMode":"strictIsolated","growthMode":"enabled","lastFeeScaleChangeTime":"2026-01-26T16:47:08.614356922","deployerFeeScale":"1.0"}],"collateralToken":0}"#;
const XYZ_CTXS: &str = r#"[{"funding":"0.00000625","openInterest":"6296.3006","prevDayPx":"30988.0","dayNtlVlm":"242279249.4440995157","premium":"0.0000479509","oraclePx":"31282.0","markPx":"31282.0","midPx":"31283.5","impactPxs":["31283.0","31284.0"],"dayBaseVlm":"7772.809"},{"funding":"0.00000625","openInterest":"134390.906","prevDayPx":"378.64","dayNtlVlm":"19647555.7842699885","premium":"0.0002571031","oraclePx":"381.17","markPx":"381.29","midPx":"381.285","impactPxs":["381.246","381.29"],"dayBaseVlm":"51580.309"},{"funding":"0.00000625","openInterest":"315305.61","prevDayPx":"112.81","dayNtlVlm":"6744808.3558599986","premium":"-0.0001060258","oraclePx":"113.18","markPx":"113.18","midPx":"113.175","impactPxs":["113.156","113.18"],"dayBaseVlm":"59156.604"},{"funding":"0.0","openInterest":"0.0","prevDayPx":"85.0","dayNtlVlm":"0.0","premium":null,"oraclePx":"85.0","markPx":"85.0","midPx":null,"impactPxs":null,"dayBaseVlm":"0.0"}]"#;
/// The whole main-dex `meta` from `allPerpMetas`, live, mainnet, 2026-09-27: all 234 perps.
const MAINNET: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/perps/meta_mainnet.json"));

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
    let (asset, btc) = meta.find(0, "BTC").unwrap();
    assert_eq!((asset.id(), btc.sz_decimals, btc.max_leverage), (0, 5, 40));
    assert_eq!(btc.grid(), Grid::perp(5));
    assert_eq!((btc.growth_mode.as_deref(), btc.deployer_fee_scale), (None, None), "HIP-3 fields only");
    assert_eq!(meta.find(0, "ATOM").unwrap().0, Asset::perp(2).unwrap());
    assert_eq!(meta.find(0, "btc"), None, "names are matched as the venue spells them");
    assert_eq!(meta.get(0, Asset::perp(1).unwrap()).map(|perp| perp.name.as_str()), Some("ETH"));
    assert_eq!(meta.get(0, Asset::from_id(10_001)), None, "a spot id is not a perp");
    assert_eq!(meta.get(1, Asset::perp(1).unwrap()), None, "a main-dex id is not on dex 1");
    let hpos = meta.find(0, "HPOS").unwrap().1;
    assert!(hpos.is_delisted && hpos.only_isolated && !hpos.allows_cross());
    assert!(btc.allows_cross());
}

#[test]
fn contexts_line_up_with_the_universe_and_a_quiet_market_has_no_mid() {
    let both = both();
    let (asset, _, btc) = both.find(0, "BTC").unwrap();
    assert_eq!(asset.id(), 0);
    assert_eq!((btc.mark_px, btc.mid_px, btc.reference()), (dec("85634"), Some(dec("85634.5")), dec("85634.5")));
    assert_eq!(btc.funding, dec("0.0000125"));
    assert_eq!(btc.impact_pxs, Some(vec![dec("85631.1"), dec("85635")]));
    let (_, _, hpos) = both.find(0, "HPOS").unwrap();
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
    assert_eq!(both.find(0, "BTC").unwrap().2.day_change(), Some(dec("0.41")));
}

#[test]
fn a_market_order_goes_out_on_the_perp_s_grid_as_frontend_market() {
    let both = both();
    let (asset, btc, ctx) = both.find(0, "BTC").unwrap();
    let order = btc.grid().market(asset, Side::Buy, ctx.reference(), dec("0.01"), dec("0.001234")).unwrap();
    assert_eq!((order.price(), order.size(), order.tif()), (dec("86490"), dec("0.00123"), Some(Tif::FrontendMarket)));
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
    let both = block_on(meta_and_asset_ctxs(&transport, Dex::Main)).unwrap().unwrap();
    assert_eq!(both.ctxs.len(), 4);
    assert_eq!(transport.sent.borrow()[0], ("/info".to_owned(), r#"{"type":"metaAndAssetCtxs"}"#.to_owned()));
    assert_eq!(meta_request(Dex::Main), r#"{"type":"meta"}"#);
}

#[test]
fn a_per_dex_read_names_its_dex() {
    let transport = Recorded { answer: "null".to_owned(), sent: RefCell::new(Vec::new()) };
    assert_eq!(block_on(meta(&transport, Dex::Hip3("zzzz"))).unwrap(), None, "a dex the venue does not know");
    assert_eq!(block_on(meta_and_asset_ctxs(&transport, Dex::Hip3("zzzz"))).unwrap(), None);
    let sent: Vec<String> = transport.sent.borrow().iter().map(|(_, body)| body.clone()).collect();
    assert_eq!(sent, [r#"{"type":"meta","dex":"zzzz"}"#, r#"{"type":"metaAndAssetCtxs","dex":"zzzz"}"#]);
}

#[test]
fn a_dex_s_perps_count_from_its_own_base() {
    let meta: Meta = serde_json::from_str(XYZ).unwrap();
    let (asset, tsla) = meta.find(1, "xyz:TSLA").unwrap();
    assert_eq!(asset.id(), 110_001);
    assert_eq!((tsla.sz_decimals, tsla.max_leverage, tsla.grid()), (3, 20, Grid::perp(3)));
    assert_eq!(tsla.deployer_fee_scale, Some(Dec::ONE));
    assert_eq!(tsla.growth_mode.as_deref(), Some("enabled"));
    assert_eq!(meta.find(2, "xyz:TSLA").unwrap().0.id(), 120_001, "the dex's place decides the id");
    assert_eq!(meta.find(1, "TSLA"), None, "a HIP-3 coin carries its dex");
    assert_eq!(meta.get(1, asset), Some(tsla));
    assert_eq!(meta.get(2, asset), None, "an id of another dex");
    let (_, hood) = meta.find(1, "xyz:HOOD").unwrap();
    assert!(!hood.allows_cross() && tsla.allows_cross());
    assert_eq!(meta.collateral_token, Some(0));
    assert_eq!(serde_json::from_str::<Option<Meta>>("null").unwrap(), None, "a dex the venue does not know");
}

#[test]
fn an_order_and_leverage_on_a_hip3_perp_name_its_asset_id() {
    let both: MetaAndAssetCtxs = serde_json::from_str(&format!("[{XYZ},{XYZ_CTXS}]")).unwrap();
    let (asset, tsla, ctx) = both.find(1, "xyz:TSLA").unwrap();
    assert_eq!((ctx.mark_px, ctx.reference()), (dec("381.29"), dec("381.285")));
    assert_eq!(both.ctx(1, asset), Some(ctx));
    let order = tsla.grid().limit(asset, Side::Sell, dec("340.187"), dec("2.5"), Tif::Gtc).unwrap();
    assert_eq!(
        PlaceOrder::one(order).wire(),
        r#"{"type":"order","orders":[{"a":110001,"b":false,"p":"340.19","s":"2.5","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#,
        "a sell rounds up"
    );
    let leverage = UpdateLeverage::new(asset, MarginMode::Isolated, 10).unwrap();
    assert_eq!(leverage.wire(), r#"{"type":"updateLeverage","asset":110001,"isCross":false,"leverage":10}"#);
    let (_, _, uranium) = both.find(1, "xyz:URANIUM").unwrap();
    assert_eq!((uranium.mid_px, uranium.reference()), (None, dec("85")));
}

/// `kPEPE` is a thousand PEPE. Of the 234 perps, the seven that start with a lower case `k` are
/// thousands; `KAS` and `KAITO` are not.
#[test]
fn a_thousands_perp_names_its_underlying_and_multiplier() {
    let meta: Meta = serde_json::from_str(MAINNET).unwrap();
    let thousands: Vec<&str> = meta
        .perps(0)
        .filter(|(_, perp)| underlying(&perp.name).1 == Dec::from_int(1000))
        .map(|(_, perp)| underlying(&perp.name).0)
        .collect();
    assert_eq!(thousands, ["PEPE", "SHIB", "BONK", "LUNC", "FLOKI", "DOGS", "NEIRO"]);
    assert_eq!(underlying("KAITO"), ("KAITO", Dec::ONE));
    assert_eq!(underlying("BTC"), ("BTC", Dec::ONE));
    assert_eq!(underlying("xyz:TSLA"), ("TSLA", Dec::ONE), "a HIP-3 coin's dex is not its underlying");
    assert_eq!(underlying("k"), ("k", Dec::ONE));
}
