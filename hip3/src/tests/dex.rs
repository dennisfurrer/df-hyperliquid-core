use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{L1Action, TransportError};
use hl_trade::{MarginMode, PlaceOrder, Side, Tif, UpdateLeverage};

use super::*;

/// Live, mainnet, 2026-10-06, the lists trimmed: the main dex, then `xyz`
/// and `flx`.
const DEXES: &str = r#"[null,{"name":"xyz","fullName":"XYZ","deployer":"0x88806a71d74ad0a510b350545c9ae490912f0888","oracleUpdater":null,"feeRecipient":"0x83ffcfb1f2ad843c474b2e28df86c721cb869d3a","assetToStreamingOiCap":[["xyz:AAOI","25000000.0"],["xyz:AAPL","200000000.0"]],"subDeployers":[["registerAsset",["0x7d16f116d252db609c56d27d6c9605eb03e16657"]],["setOracle",["0x1234567890545d1df9ee64b35fdd16966e08acec"]]],"assetToFundingMultiplier":[["xyz:AAOI","0.5"],["xyz:AAPL","0.5"]],"assetToFundingInterestRate":[["xyz:EUR","0.0"]],"assetToFundingClamp":[]},{"name":"flx","fullName":"Felix Exchange","deployer":"0x2fab552502a6d45920d5741a2f3ebf4c35536352","oracleUpdater":"0x94757f8dcb4bf73b850195660e959d1105cfedd5","feeRecipient":"0xe2872b5ae7dcbba40cc4510d08c8bbea95b42d43","assetToStreamingOiCap":[["flx:COIN","8000000.0"]],"subDeployers":[["registerAsset",["0xd0d4ef34424af3da883672b8cdeca751293655a5"]]],"assetToFundingMultiplier":[["flx:COIN","0.0"]],"assetToFundingInterestRate":[],"assetToFundingClamp":[]}]"#;
/// `meta` with `dex: "xyz"` and its contexts, the same day: the first two
/// perps, HOOD (fifth live, no cross) and URANIUM (delisted, no book).
const XYZ: &str = r#"{"universe":[{"szDecimals":4,"name":"xyz:XYZ100","maxLeverage":30,"marginTableId":30,"growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:TSLA","maxLeverage":20,"marginTableId":20,"growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:HOOD","maxLeverage":10,"marginTableId":10,"onlyIsolated":true,"marginMode":"noCross","growthMode":"enabled","lastFeeScaleChangeTime":"2025-11-23T17:37:10.033211662","deployerFeeScale":"1.0"},{"szDecimals":3,"name":"xyz:URANIUM","maxLeverage":10,"marginTableId":10,"onlyIsolated":true,"isDelisted":true,"marginMode":"strictIsolated","growthMode":"enabled","lastFeeScaleChangeTime":"2026-01-26T16:47:08.614356922","deployerFeeScale":"1.0"}],"collateralToken":0}"#;
const XYZ_CTXS: &str = r#"[{"funding":"0.00000625","openInterest":"6296.3006","prevDayPx":"30988.0","dayNtlVlm":"242279249.4440995157","premium":"0.0000479509","oraclePx":"31282.0","markPx":"31282.0","midPx":"31283.5","impactPxs":["31283.0","31284.0"],"dayBaseVlm":"7772.809"},{"funding":"0.00000625","openInterest":"134390.906","prevDayPx":"378.64","dayNtlVlm":"19647555.7842699885","premium":"0.0002571031","oraclePx":"381.17","markPx":"381.29","midPx":"381.285","impactPxs":["381.246","381.29"],"dayBaseVlm":"51580.309"},{"funding":"0.00000625","openInterest":"315305.61","prevDayPx":"112.81","dayNtlVlm":"6744808.3558599986","premium":"-0.0001060258","oraclePx":"113.18","markPx":"113.18","midPx":"113.175","impactPxs":["113.156","113.18"],"dayBaseVlm":"59156.604"},{"funding":"0.0","openInterest":"0.0","prevDayPx":"85.0","dayNtlVlm":"0.0","premium":null,"oraclePx":"85.0","markPx":"85.0","midPx":null,"impactPxs":null,"dayBaseVlm":"0.0"}]"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_dex_s_place_in_perp_dexs_is_the_dex_in_its_asset_ids() {
    let dexes: PerpDexs = serde_json::from_str(DEXES).unwrap();
    assert_eq!(dexes.index_of("xyz"), Some(1));
    assert_eq!(dexes.index_of("flx"), Some(2));
    assert_eq!(dexes.index_of(""), None, "the main dex is not a HIP-3 dex");
    let xyz = dexes.get("xyz").unwrap();
    assert_eq!(xyz.full_name, "XYZ");
    assert_eq!(xyz.oracle_updater, None);
    assert_eq!(xyz.deployer.unwrap().to_hex(), "0x88806a71d74ad0a510b350545c9ae490912f0888");
    assert_eq!(xyz.sub_deployers[1].0, "setOracle");
    assert_eq!(xyz.asset_to_streaming_oi_cap[1], ("xyz:AAPL".to_owned(), dec("200000000")));
    assert_eq!(dexes.dexes().count(), 2);
}

#[test]
fn a_dex_s_perps_count_from_its_own_base() {
    let meta: DexMeta = serde_json::from_str(XYZ).unwrap();
    let (asset, tsla) = meta.find(1, "xyz:TSLA").unwrap();
    assert_eq!(asset.id(), 110_001);
    assert_eq!((tsla.sz_decimals, tsla.max_leverage, tsla.grid()), (3, 20, Grid::perp(3)));
    assert_eq!(tsla.deployer_fee_scale, Some(Dec::ONE));
    assert_eq!(tsla.growth_mode.as_deref(), Some("enabled"));
    assert_eq!(meta.find(2, "xyz:TSLA").unwrap().0.id(), 120_001, "the dex's place decides the id");
    assert_eq!(meta.find(1, "TSLA"), None, "a HIP-3 coin carries its dex");
    let (_, hood) = meta.find(1, "xyz:HOOD").unwrap();
    assert!(!hood.allows_cross() && tsla.allows_cross());
    assert_eq!(meta.collateral_token, Some(0));
    assert_eq!(serde_json::from_str::<Option<DexMeta>>("null").unwrap(), None, "a dex the venue does not know");
}

#[test]
fn an_order_and_leverage_on_a_hip3_perp_name_its_asset_id() {
    let both: DexMetaAndAssetCtxs = serde_json::from_str(&format!("[{XYZ},{XYZ_CTXS}]")).unwrap();
    let (asset, tsla, ctx) = both.find(1, "xyz:TSLA").unwrap();
    assert_eq!((ctx.mark_px, ctx.reference()), (dec("381.29"), dec("381.285")));
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

struct Recorded {
    sent: RefCell<Vec<String>>,
}

impl Transport for Recorded {
    fn post(&self, _: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push(body);
        ready(Ok("null".to_owned()))
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
fn a_per_dex_read_names_its_dex() {
    let transport = Recorded { sent: RefCell::new(Vec::new()) };
    assert_eq!(block_on(dex_meta(&transport, "zzzz")).unwrap(), None);
    assert_eq!(block_on(dex_meta_and_asset_ctxs(&transport, "zzzz")).unwrap(), None);
    assert_eq!(
        transport.sent.borrow().as_slice(),
        [r#"{"type":"meta","dex":"zzzz"}"#.to_owned(), r#"{"type":"metaAndAssetCtxs","dex":"zzzz"}"#.to_owned()]
    );
    assert_eq!(perp_dexs_request(), r#"{"type":"perpDexs"}"#);
}
