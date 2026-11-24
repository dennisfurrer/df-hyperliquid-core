//! Live answers from 2026-09-26 and 27, stored as sent: `perpDexs`, `allPerpMetas` (all 11 dexes,
//! 528 perps), `perpDexLimits` and `perpDexStatus` for `xyz`, and `metaAndAssetCtxs` for `para`, all
//! mainnet; `perpCategories` from 2026-10-21, whole; and `perpConciseAnnotations` and two
//! `perpAnnotation` reads from 2026-11-24, whole. Reads through a client are in
//! `tests/mocked/markets`.

use crate::markets::dex::split_coin;
use crate::markets::dexes::annotations::{ConciseAnnotation, PerpAnnotation};
use crate::markets::dexes::dex::PerpDexs;
use crate::markets::dexes::limits::{DexLimits, DexStatus};
use crate::markets::dexes::listing::{Dexes, Misaligned};
use crate::markets::perps::{Meta, MetaAndAssetCtxs};
use crate::utils::{Address, Asset, AssetKind, Dec};

const DEXES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_dexs_mainnet.json"));
const METAS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/all_perp_metas_mainnet.json"));
const LIMITS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_dex_limits_xyz_mainnet.json"));
const STATUS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_dex_status_xyz_mainnet.json"));
const PARA: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/meta_and_asset_ctxs_para_mainnet.json"));
const CATEGORIES: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_categories_mainnet.json"));
const CONCISE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_concise_annotations_mainnet.json"));
const XYZ100: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_annotation_xyz100_mainnet.json"));
const BTC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/hip3/perp_annotation_btc_mainnet.json"));

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn list() -> PerpDexs {
    serde_json::from_str(DEXES).unwrap()
}

fn metas() -> Vec<Meta> {
    serde_json::from_str(METAS).unwrap()
}

fn joined() -> Dexes {
    Dexes::new(list(), metas()).unwrap()
}

/// `para` clamps its three rates perps, the only clamps on mainnet; the docs allow 0 to 0.01.
#[test]
fn every_dex_lines_up_with_its_meta_and_names_its_deployers_and_funding_settings() {
    let dexes = joined();
    let names: Vec<&str> = dexes.dexes().map(|(_, dex, _)| dex.name.as_str()).collect();
    assert_eq!(names, ["xyz", "flx", "vntl", "hyna", "km", "abcd", "cash", "para", "mkts", "io"]);
    assert_eq!((dexes.metas.len(), dexes.metas[0].universe[0].name.as_str()), (11, "BTC"), "the main dex's first");
    let (place, flx, meta) = dexes.dex("flx").unwrap();
    assert_eq!(
        (place, flx.full_name.as_str(), meta.collateral_token, meta.universe.len()),
        (2, "Felix Exchange", Some(360), 16)
    );
    assert_eq!(dexes.dex("hyna").and_then(|(_, _, meta)| meta.collateral_token), Some(235));
    assert_eq!((dexes.list.index_of("xyz"), dexes.list.index_of("")), (Some(1), None), "the main dex is no HIP-3 dex");
    assert_eq!((dexes.list.name_of(8), dexes.list.name_of(0), dexes.list.name_of(99)), (Some("para"), None, None));
    let xyz = dexes.list.get("xyz").unwrap();
    assert_eq!((xyz.full_name.as_str(), xyz.oracle_updater), ("XYZ", None));
    assert_eq!(xyz.deployer.unwrap().to_hex(), "0x88806a71d74ad0a510b350545c9ae490912f0888");
    let register: Address = "0x7d16f116d252db609c56d27d6c9605eb03e16657".parse().unwrap();
    assert_eq!((xyz.sub_deployers.len(), xyz.sub_deployers_for("registerAsset").len()), (11, 3));
    assert!(xyz.sub_deployers_for("registerAsset").contains(&register));
    assert_eq!(xyz.sub_deployers_for("disableDex"), []);
    assert_eq!((xyz.funding_interest_rate("xyz:EUR"), xyz.asset_to_funding_interest_rate.len()), (Some(Dec::ZERO), 4));
    assert_eq!(xyz.streaming_oi_cap("xyz:AAPL"), Some(dec("200000000")));
    let (_, para, _) = dexes.dex("para").unwrap();
    let clamps: Vec<(&str, Dec)> =
        para.asset_to_funding_clamp.iter().map(|(coin, clamp)| (coin.as_str(), *clamp)).collect();
    assert_eq!(clamps, [("para:10Y", dec("0.005")), ("para:2Y", dec("0.005")), ("para:30Y", dec("0.005"))]);
    assert_eq!((para.funding_clamp("para:2Y"), para.funding_clamp("para:TOTAL2")), (Some(dec("0.005")), None));
    assert_eq!(
        (para.funding_multiplier("para:10Y"), para.funding_multiplier("para:TOTAL2")),
        (Some(dec("0.1")), Some(dec("0.6")))
    );
    assert_eq!(para.funding_multiplier("para:ZZZ"), None, "unlisted is 1");
    let clamped: Vec<&str> = dexes
        .dexes()
        .filter(|(_, dex, _)| !dex.asset_to_funding_clamp.is_empty())
        .map(|(_, dex, _)| dex.name.as_str())
        .collect();
    assert_eq!(clamped, ["para"]);
    let (_, abcd, _) = dexes.dex("abcd").unwrap();
    assert!(abcd.sub_deployers.is_empty() && abcd.fee_recipient.is_none());
}

#[test]
fn a_meta_out_of_line_with_its_dex_is_refused() {
    let short = metas()[..3].to_vec();
    assert_eq!(Dexes::new(list(), short), Err(Misaligned::Count { dexes: 11, metas: 3 }));
    let mut swapped = metas();
    swapped.swap(1, 2);
    let refused = Dexes::new(list(), swapped);
    assert_eq!(refused, Err(Misaligned::Coin { coin: "flx:TSLA".to_owned(), dex: "xyz".to_owned() }));
    assert_eq!(refused.unwrap_err().to_string(), "flx:TSLA is listed under dex xyz");
}

/// A HIP-3 perp's asset id is `100000 + place * 10000 + index`: `xyz:TSLA` is 110001 and `flx`'s
/// first perp 120000.
#[test]
fn a_coin_and_its_asset_id_name_each_other() {
    let dexes = joined();
    for (coin, id, place, index) in
        [("xyz:TSLA", 110_001, 1, 1), ("flx:TSLA", 120_000, 2, 0), ("para:10Y", 180_015, 8, 15)]
    {
        let (asset, perp) = dexes.find(coin).unwrap();
        assert_eq!(
            (asset.id(), asset.kind(), asset.hip3_parts()),
            (id, AssetKind::Hip3Perp, Some((place, index))),
            "{coin}"
        );
        let (dex, found) = dexes.get(asset).unwrap();
        assert_eq!((dex.name.as_str(), found), (coin.split_once(':').unwrap().0, perp), "{coin}");
    }
    for coin in ["TSLA", "zzz:TSLA", "xyz:ZZZ"] {
        assert_eq!(dexes.find(coin), None, "{coin}");
    }
    for id in [0, 100_000, 199_999] {
        assert_eq!(dexes.get(Asset::from_id(id)), None, "{id}");
    }
}

/// The caps `perpDexLimits` lists for `xyz` are the ones `perpDexs` streams its coins toward, and
/// `metaAndAssetCtxs` for `para` gives the meta `allPerpMetas` lists at its place: reads that agree.
#[test]
fn a_dex_s_limits_status_and_contexts_agree_with_its_listing() {
    let dexes = joined();
    let limits: DexLimits = serde_json::from_str(LIMITS).unwrap();
    assert_eq!(
        (limits.total_oi_cap, limits.oi_sz_cap_per_perp, limits.max_transfer_ntl),
        (dec("10000000000"), dec("20000000000"), dec("3000000000"))
    );
    assert_eq!((limits.coin_cap("xyz:AAPL"), limits.coin_cap("BTC")), (Some(dec("200000000")), None));
    let (_, xyz, _) = dexes.dex("xyz").unwrap();
    assert_eq!((&limits.coin_to_oi_cap, limits.coin_to_oi_cap.len()), (&xyz.asset_to_streaming_oi_cap, 123));
    let status: DexStatus = serde_json::from_str(STATUS).unwrap();
    assert_eq!(status.total_net_deposit, dec("1091195769.8109180927"));
    let para: MetaAndAssetCtxs = serde_json::from_str(PARA).unwrap();
    let (place, _, meta) = dexes.dex("para").unwrap();
    assert_eq!((&para.meta, para.ctxs.len()), (meta, para.meta.universe.len()));
    let (asset, ten_year, ctx) = para.find(place, "para:10Y").unwrap();
    assert_eq!((asset.id(), ten_year.max_leverage), (180_015, 20));
    assert_eq!((ctx.mark_px, ctx.mid_px, ctx.premium), (dec("5.167"), Some(dec("5.1675")), Some(dec("-0.0001286765"))));
    assert_eq!(
        (ctx.impact_pxs.clone(), ctx.day_base_vlm),
        (Some(vec![dec("5.16283"), dec("5.17184")]), Some(dec("1005.4")))
    );
}

/// Every category names a HIP-3 perp, once, on a dex `perpDexs` lists; none is the main dex's. The
/// words are kept as the deployers wrote them.
#[test]
fn each_hip3_perp_s_category_is_read_by_its_coin_as_written() {
    let categories: Vec<(String, String)> = serde_json::from_str(CATEGORIES).unwrap();
    assert_eq!(categories.len(), 222);
    let dexes = list();
    for (coin, _) in &categories {
        let (dex, _) = split_coin(coin).unwrap_or_else(|| panic!("{coin} is on the main dex"));
        assert!(dexes.get(dex).is_some(), "{coin}");
        assert_eq!(categories.iter().filter(|(other, _)| other == coin).count(), 1, "{coin}");
    }
    let of = |coin: &str| categories.iter().find(|(listed, _)| listed == coin).map(|(_, category)| category.as_str());
    assert_eq!((of("xyz:TSLA"), of("xyz:EUR"), of("km:EUR")), (Some("stocks"), Some("fx"), Some("FX")));
    assert_eq!((of("para:AAOI"), of("BTC")), (Some("stock"), None));
}

/// Every concise annotation names a HIP-3 perp once. A display name and keywords are optional, so
/// some annotations are a category alone.
#[test]
fn each_hip3_perp_s_concise_annotation_is_read_by_its_coin() {
    let annotations: Vec<(String, ConciseAnnotation)> = serde_json::from_str(CONCISE).unwrap();
    assert_eq!(annotations.len(), 239);
    for (coin, _) in &annotations {
        assert!(split_coin(coin).is_some(), "{coin} is on the main dex");
        assert_eq!(annotations.iter().filter(|(other, _)| other == coin).count(), 1, "{coin}");
    }
    let of = |coin: &str| annotations.iter().find(|(listed, _)| listed == coin).map(|(_, annotation)| annotation);
    let aapl = of("io:AAPL").unwrap();
    assert_eq!((aapl.category.as_str(), aapl.display_name.as_deref()), ("stocks", Some("AAPL")));
    assert_eq!(aapl.keywords, ["apple", "aapl"]);
    let btc = of("flx:BTC").unwrap();
    assert_eq!((btc.category.as_str(), btc.display_name.as_deref(), btc.keywords.len()), ("crypto", None, 0));
    assert!(
        annotations.iter().any(|(_, annotation)| annotation.display_name.is_none() && !annotation.keywords.is_empty())
    );
}

/// One perp's annotation carries its description; a main dex perp has none and reads as `None`.
#[test]
fn a_perp_s_annotation_reads_with_its_description_and_a_main_dex_perp_has_none() {
    let xyz100: Option<PerpAnnotation> = serde_json::from_str(XYZ100).unwrap();
    let xyz100 = xyz100.unwrap();
    assert_eq!((xyz100.category.as_str(), xyz100.display_name.as_deref()), ("indices", Some("XYZ100")));
    assert!(xyz100.description.starts_with("XYZ100 tracks a modified market-capitalization-weighted index"));
    assert_eq!(xyz100.keywords, ["nasdaq", "qqq"]);
    let btc: Option<PerpAnnotation> = serde_json::from_str(BTC).unwrap();
    assert_eq!(btc, None);
}
