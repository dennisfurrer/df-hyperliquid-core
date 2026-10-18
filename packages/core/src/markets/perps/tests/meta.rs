//! `meta` and `metaAndAssetCtxs` as the venue answered them on mainnet, for the main dex and for
//! `xyz`, and what an order on a perp names from them. Reads through a client are in
//! `tests/mocked/markets`.

use crate::trade::{MarginMode, Order, PlaceOrder, Side, Tif, UpdateLeverage};
use crate::trade::{add_margin, remove_margin};
use crate::utils::Dec;
use crate::utils::sign::L1Action;

use super::*;

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
fn meta_names_each_perp_s_asset_id_and_grid() {
    let meta: Meta = serde_json::from_str(META).unwrap();
    assert_eq!((meta.universe.len(), meta.collateral_token), (4, Some(0)));
    let (asset, btc) = meta.find(0, "BTC").unwrap();
    assert_eq!((asset.id(), btc.sz_decimals, btc.max_leverage, btc.grid()), (0, 5, 40, Grid::perp(5)));
    assert_eq!((btc.growth_mode.as_deref(), btc.deployer_fee_scale), (None, None), "HIP-3 fields only");
    assert_eq!(meta.find(0, "ATOM").unwrap().0, Asset::perp(2).unwrap());
    assert_eq!(meta.find(0, "btc"), None, "names are matched as the venue spells them");
    let gets = [
        (0, Asset::perp(1).unwrap(), Some("ETH")),
        (0, Asset::from_id(10_001), None),
        (1, Asset::perp(1).unwrap(), None),
        (0, Asset::perp(9).unwrap(), None),
    ];
    for (place, asset, name) in gets {
        assert_eq!(meta.get(place, asset).map(|perp| perp.name.as_str()), name, "{} on dex {place}", asset.id());
    }
    let hpos = meta.find(0, "HPOS").unwrap().1;
    assert!(hpos.is_delisted && hpos.only_isolated && !hpos.allows_cross() && btc.allows_cross());
    assert_eq!(meta.perps(0).count(), 4, "delisted perps keep their place");
}

#[test]
fn contexts_line_up_with_the_universe_and_bound_a_market_order_on_the_perp_s_grid() {
    let both = both();
    let (asset, btc, ctx) = both.find(0, "BTC").unwrap();
    assert_eq!((ctx.mark_px, ctx.mid_px, ctx.reference()), (dec("85634"), Some(dec("85634.5")), dec("85634.5")));
    assert_eq!((ctx.funding, ctx.day_change()), (dec("0.0000125"), Some(dec("0.41"))));
    assert_eq!(both.ctx(0, asset), Some(ctx));
    let (_, _, hpos) = both.find(0, "HPOS").unwrap();
    assert_eq!((hpos.mid_px, hpos.premium, hpos.reference()), (None, None, dec("0.041873")), "a quiet market");
    assert_eq!(both.ctx(0, Asset::perp(9).unwrap()), None);
    assert_eq!(both.meta.margin_table(50).and_then(|table| table.max_leverage_at(Dec::ZERO)), Some(50));
    let bound = btc.grid().market_bound(ctx.reference(), dec("0.01"), Side::Buy).unwrap();
    let order = Order::market(asset, Side::Buy, bound, btc.grid().size(dec("0.001234")).unwrap());
    assert_eq!((order.price(), order.size(), order.tif()), (dec("86490"), dec("0.00123"), Some(Tif::FrontendMarket)));
}

/// `xyz` at place 1 counts its perps from 110000; at place 2 the same list would count from
/// 120000, so the place decides the id an order signs.
#[test]
fn a_hip3_perp_counts_from_its_dex_s_base_and_an_order_names_that_id() {
    let both: MetaAndAssetCtxs = serde_json::from_str(&format!("[{XYZ},{XYZ_CTXS}]")).unwrap();
    let (asset, tsla, ctx) = both.find(1, "xyz:TSLA").unwrap();
    assert_eq!((asset.id(), tsla.sz_decimals, tsla.max_leverage, tsla.grid()), (110_001, 3, 20, Grid::perp(3)));
    assert_eq!((tsla.deployer_fee_scale, tsla.growth_mode.as_deref()), (Some(Dec::ONE), Some("enabled")));
    assert_eq!((ctx.mark_px, ctx.reference(), both.ctx(1, asset)), (dec("381.29"), dec("381.285"), Some(ctx)));
    let meta = &both.meta;
    assert_eq!(meta.find(2, "xyz:TSLA").unwrap().0.id(), 120_001, "the dex's place decides the id");
    assert_eq!((meta.find(1, "TSLA"), meta.get(2, asset)), (None, None), "a HIP-3 coin carries its dex");
    assert_eq!((meta.get(1, asset), both.ctx(2, asset)), (Some(tsla), None));
    assert!(!meta.find(1, "xyz:HOOD").unwrap().1.allows_cross() && tsla.allows_cross(), "noCross");
    let (_, _, uranium) = both.find(1, "xyz:URANIUM").unwrap();
    assert_eq!((uranium.mid_px, uranium.reference()), (None, dec("85")));
    let price = tsla.grid().price(dec("340.187"), Side::Sell).unwrap();
    let order = Order::limit(asset, Side::Sell, price, tsla.grid().size(dec("2.5")).unwrap(), Tif::Gtc);
    assert_eq!(
        PlaceOrder::one(order).wire(),
        r#"{"type":"order","orders":[{"a":110001,"b":false,"p":"340.19","s":"2.5","r":false,"t":{"limit":{"tif":"Gtc"}}}],"grouping":"na"}"#,
        "a sell rounds up"
    );
    let leverage = UpdateLeverage::new(asset, MarginMode::Isolated, 10).unwrap();
    assert_eq!(leverage.wire(), r#"{"type":"updateLeverage","asset":110001,"isCross":false,"leverage":10}"#);
    assert_eq!(serde_json::from_str::<Option<Meta>>("null").unwrap(), None, "a dex the venue does not know");
}

/// Isolated margin is added as a positive amount and removed as a negative one, in millionths.
#[test]
fn margin_goes_in_positive_and_comes_out_negative() {
    let eth = Asset::perp(1).unwrap();
    assert_eq!(add_margin(eth, Side::Buy, dec("12.5")).unwrap().ntli(), 12_500_000);
    assert_eq!(remove_margin(eth, Side::Sell, dec("2.5")).unwrap().ntli(), -2_500_000);
    assert!(add_margin(eth, Side::Buy, dec("-1")).is_err(), "a negative add is a removal, said as one");
    assert!(remove_margin(eth, Side::Buy, Dec::ZERO).is_err());
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
    for (coin, tracks) in [("KAITO", "KAITO"), ("BTC", "BTC"), ("xyz:TSLA", "TSLA"), ("k", "k")] {
        assert_eq!(underlying(coin), (tracks, Dec::ONE), "{coin}");
    }
}
