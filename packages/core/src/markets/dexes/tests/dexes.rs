//! Live answers from 2026-09-26 and 27, stored as sent: `perpDexs`, `allPerpMetas` (all 11 dexes,
//! 528 perps), `perpDexLimits` and `perpDexStatus` for `xyz`, and `metaAndAssetCtxs` for `para`, all
//! mainnet.

use crate::markets::Dex;
use crate::markets::dexes::dex::PerpDexs;
use crate::markets::dexes::limits::{
    DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
};
use crate::markets::dexes::listing::{Dexes, Misaligned, all_perp_metas_request, dexes};
use crate::markets::dexes::support::{Recorded, block_on, dec};
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

fn list() -> PerpDexs {
    serde_json::from_str(DEXES).unwrap()
}

fn metas() -> Vec<Meta> {
    serde_json::from_str(METAS).unwrap()
}

fn joined() -> Dexes {
    Dexes::new(list(), metas()).unwrap()
}

#[test]
fn every_dex_lines_up_with_its_meta() {
    let dexes = joined();
    let names: Vec<&str> = dexes.dexes().map(|(_, dex, _)| dex.name.as_str()).collect();
    assert_eq!(names, ["xyz", "flx", "vntl", "hyna", "km", "abcd", "cash", "para", "mkts", "io"]);
    let (place, flx, meta) = dexes.dex("flx").unwrap();
    assert_eq!((place, flx.full_name.as_str(), meta.collateral_token), (2, "Felix Exchange", Some(360)));
    assert_eq!(meta.universe.len(), 16);
    assert_eq!(dexes.dex("hyna").and_then(|(_, _, meta)| meta.collateral_token), Some(235));
    assert_eq!(dexes.metas.len(), 11, "the main dex's meta first");
    assert_eq!(dexes.metas[0].universe[0].name, "BTC");
    assert_eq!((dexes.list.index_of("xyz"), dexes.list.index_of("flx")), (Some(1), Some(2)));
    assert_eq!(dexes.list.index_of(""), None, "the main dex is not a HIP-3 dex");
    let xyz = dexes.list.get("xyz").unwrap();
    assert_eq!((xyz.full_name.as_str(), xyz.oracle_updater), ("XYZ", None));
    assert_eq!(xyz.deployer.unwrap().to_hex(), "0x88806a71d74ad0a510b350545c9ae490912f0888");
}

#[test]
fn a_meta_out_of_line_with_its_dex_is_refused() {
    let short = metas()[..3].to_vec();
    assert_eq!(Dexes::new(list(), short), Err(Misaligned::Count { dexes: 11, metas: 3 }));
    let mut swapped = metas();
    swapped.swap(1, 2);
    assert_eq!(
        Dexes::new(list(), swapped),
        Err(Misaligned::Coin { coin: "flx:TSLA".to_owned(), dex: "xyz".to_owned() })
    );
}

/// A HIP-3 perp's asset id is `100000 + place * 10000 + index`: `xyz:TSLA` is 110001 and `flx`'s
/// first perp 120000.
#[test]
fn a_coin_and_its_asset_id_name_each_other() {
    let dexes = joined();
    let (asset, tsla) = dexes.find("xyz:TSLA").unwrap();
    assert_eq!((asset.id(), tsla.sz_decimals), (110_001, 3));
    assert_eq!(dexes.find("flx:TSLA").map(|(asset, _)| asset.id()), Some(120_000));
    let (asset, ten_year) = dexes.find("para:10Y").unwrap();
    assert_eq!(asset.hip3_parts(), Some((8, 15)));
    assert_eq!(asset.kind(), AssetKind::Hip3Perp);
    let (dex, found) = dexes.get(asset).unwrap();
    assert_eq!((dex.name.as_str(), found), ("para", ten_year));
    assert_eq!(dexes.find("TSLA"), None, "a HIP-3 coin carries its dex");
    assert_eq!(dexes.find("zzz:TSLA"), None);
    assert_eq!(dexes.get(Asset::from_id(0)), None, "a main-dex id is not a HIP-3 one");
    assert_eq!(dexes.get(Asset::from_id(100_000)), None, "place 0 is the main dex");
    assert_eq!(dexes.list.name_of(8), Some("para"));
    assert_eq!(dexes.list.name_of(0), None);
}

/// `para` clamps its three rates perps, the only clamps on mainnet; the docs allow 0 to 0.01.
#[test]
fn a_dex_names_its_funding_clamps_multipliers_and_rates() {
    let dexes = joined();
    let (_, para, _) = dexes.dex("para").unwrap();
    assert_eq!(
        para.asset_to_funding_clamp,
        [
            ("para:10Y".to_owned(), dec("0.005")),
            ("para:2Y".to_owned(), dec("0.005")),
            ("para:30Y".to_owned(), dec("0.005"))
        ]
    );
    assert_eq!(para.funding_clamp("para:2Y"), Some(dec("0.005")));
    assert_eq!(para.funding_clamp("para:TOTAL2"), None);
    assert_eq!(para.funding_multiplier("para:10Y"), Some(dec("0.1")));
    let clamped: Vec<&str> = dexes
        .dexes()
        .filter(|(_, dex, _)| !dex.asset_to_funding_clamp.is_empty())
        .map(|(_, dex, _)| dex.name.as_str())
        .collect();
    assert_eq!(clamped, ["para"]);
    let ceiling = dec("0.01");
    assert!(dexes.dexes().flat_map(|(_, dex, _)| &dex.asset_to_funding_clamp).all(|(_, clamp)| *clamp <= ceiling));
    let (_, xyz, _) = dexes.dex("xyz").unwrap();
    assert_eq!(xyz.funding_interest_rate("xyz:EUR"), Some(Dec::ZERO));
    assert_eq!(xyz.asset_to_funding_interest_rate.len(), 4);
    assert_eq!(xyz.streaming_oi_cap("xyz:AAPL"), Some(dec("200000000")));
}

#[test]
fn a_dex_names_who_may_send_each_deploy_action() {
    let dexes = joined();
    let (_, xyz, _) = dexes.dex("xyz").unwrap();
    assert_eq!(xyz.sub_deployers.len(), 11);
    let register: Address = "0x7d16f116d252db609c56d27d6c9605eb03e16657".parse().unwrap();
    assert_eq!(xyz.sub_deployers_for("registerAsset").len(), 3);
    assert!(xyz.sub_deployers_for("registerAsset").contains(&register));
    assert_eq!(xyz.sub_deployers_for("disableDex"), []);
    let (_, abcd, _) = dexes.dex("abcd").unwrap();
    assert!(abcd.sub_deployers.is_empty() && abcd.fee_recipient.is_none());
}

/// The caps `perpDexLimits` lists for `xyz` are the ones `perpDexs` streams its coins toward: two
/// reads that agree, all 123 of them.
#[test]
fn a_dex_s_limits_and_its_status_read_as_sent() {
    let limits: DexLimits = serde_json::from_str(LIMITS).unwrap();
    assert_eq!(limits.total_oi_cap, dec("10000000000"));
    assert_eq!(limits.oi_sz_cap_per_perp, dec("20000000000"));
    assert_eq!(limits.max_transfer_ntl, dec("3000000000"));
    assert_eq!(limits.coin_cap("xyz:AAPL"), Some(dec("200000000")));
    let dexes = joined();
    let (_, xyz, _) = dexes.dex("xyz").unwrap();
    assert_eq!(limits.coin_to_oi_cap, xyz.asset_to_streaming_oi_cap);
    assert_eq!(limits.coin_to_oi_cap.len(), 123);
    let status: DexStatus = serde_json::from_str(STATUS).unwrap();
    assert_eq!(status.total_net_deposit, dec("1091195769.8109180927"));
}

/// `metaAndAssetCtxs` with `dex: "para"` gives the meta `allPerpMetas` lists at para's place, one
/// context per perp.
#[test]
fn a_dex_s_contexts_line_up_with_its_meta() {
    let para: MetaAndAssetCtxs = serde_json::from_str(PARA).unwrap();
    let dexes = joined();
    let (place, _, meta) = dexes.dex("para").unwrap();
    assert_eq!(&para.meta, meta);
    assert_eq!(para.ctxs.len(), para.meta.universe.len());
    let (asset, ten_year, ctx) = para.find(place, "para:10Y").unwrap();
    assert_eq!((asset.id(), ten_year.max_leverage), (180_015, 20));
    assert_eq!((ctx.mark_px, ctx.mid_px, ctx.premium), (dec("5.167"), Some(dec("5.1675")), Some(dec("-0.0001286765"))));
    assert_eq!(ctx.impact_pxs, Some(vec![dec("5.16283"), dec("5.17184")]));
    assert_eq!(ctx.day_base_vlm, Some(dec("1005.4")));
}

#[test]
fn the_reads_name_their_dex() {
    assert_eq!(all_perp_metas_request(), r#"{"type":"allPerpMetas"}"#);
    assert_eq!(perp_dex_limits_request(Dex::Hip3("xyz")), r#"{"type":"perpDexLimits","dex":"xyz"}"#);
    assert_eq!(perp_dex_status_request(Dex::Hip3("xyz")), r#"{"type":"perpDexStatus","dex":"xyz"}"#);
    assert_eq!(perp_dex_status_request(Dex::Main), r#"{"type":"perpDexStatus","dex":""}"#);
    let transport = Recorded::new(&[
        ("perpDexs", DEXES),
        ("allPerpMetas", METAS),
        ("perpDexLimits", LIMITS),
        ("perpDexStatus", STATUS),
    ]);
    assert_eq!(block_on(dexes(&transport)).unwrap(), joined());
    assert_eq!(
        block_on(perp_dex_limits(&transport, Dex::Hip3("xyz"))).unwrap().map(|limits| limits.coin_to_oi_cap.len()),
        Some(123)
    );
    assert_eq!(
        block_on(perp_dex_status(&transport, Dex::Hip3("xyz"))).unwrap().map(|status| status.total_net_deposit),
        Some(dec("1091195769.8109180927"))
    );
    assert_eq!(block_on(perp_dex_limits(&transport, Dex::Main)).unwrap(), None, "the main dex has no limits: null");
    let sent = transport.sent();
    assert_eq!(sent[..2], [r#"{"type":"perpDexs"}"#, r#"{"type":"allPerpMetas"}"#], "the list, then the metas");
    assert_eq!(sent[4], r#"{"type":"perpDexLimits","dex":""}"#);
}

#[test]
fn a_misaligned_answer_is_unreadable() {
    let transport = Recorded::new(&[("perpDexs", DEXES), ("allPerpMetas", "[]")]);
    assert!(block_on(dexes(&transport)).is_err());
}
