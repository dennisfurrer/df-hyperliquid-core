//! Every HIP-3 dex, its limits, its status, its funding settings and the
//! deploy auction, against live mainnet answers from 2026-09-26 and 27 copied
//! from hip4-tui (`net/src/tests/fixtures`, and `tui/src/app/tests/fixtures`
//! for para's contexts), stored as sent: `perpDexs`, `allPerpMetas` (all 11
//! dexes, 528 perps), `perpDexLimits` and `perpDexStatus` for `xyz`,
//! `perpDeployAuctionStatus` on mainnet (ended) and testnet (running), and
//! `metaAndAssetCtxs` for `para`.

use hl_core::{Address, Asset, AssetKind, Dec};
use hl_trade::MarginMode;

use crate::{
    auction::{DeployAuction, perp_deploy_auction, perp_deploy_auction_request},
    dex::{DexMeta, DexMetaAndAssetCtxs, PerpDexs},
    ids::{asset_parts, coin, split_coin},
    limits::{
        DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
    },
    listing::{Dexes, Misaligned, all_perp_metas_request, dexes},
    margin::{LeverageError, update_leverage, update_leverage_for},
    support::{Recorded, block_on, dec},
};

const DEXES: &str = include_str!("fixtures/perp_dexs_mainnet.json");
const METAS: &str = include_str!("fixtures/all_perp_metas_mainnet.json");
const LIMITS: &str = include_str!("fixtures/perp_dex_limits_xyz_mainnet.json");
const STATUS: &str = include_str!("fixtures/perp_dex_status_xyz_mainnet.json");
const AUCTION_MAINNET: &str = include_str!("fixtures/perp_deploy_auction_mainnet.json");
const AUCTION_TESTNET: &str = include_str!("fixtures/perp_deploy_auction_testnet.json");
const PARA: &str = include_str!("fixtures/meta_and_asset_ctxs_para_mainnet.json");

fn list() -> PerpDexs {
    serde_json::from_str(DEXES).unwrap()
}

fn metas() -> Vec<DexMeta> {
    serde_json::from_str(METAS).unwrap()
}

fn joined() -> Dexes {
    Dexes::new(list(), metas()).unwrap()
}

/// hip4-tui's test of the same answers names these ten, in this order.
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

/// A HIP-3 perp's asset id is `100000 + place * 10000 + index`; hip4-tui's
/// own reader gives `xyz:TSLA` 110001 and `flx`'s first 120000.
#[test]
fn a_coin_and_its_asset_id_name_each_other() {
    let dexes = joined();
    let (asset, tsla) = dexes.find("xyz:TSLA").unwrap();
    assert_eq!((asset.id(), tsla.sz_decimals), (110_001, 3));
    assert_eq!(dexes.find("flx:TSLA").map(|(asset, _)| asset.id()), Some(120_000));
    let (asset, ten_year) = dexes.find("para:10Y").unwrap();
    assert_eq!(asset_parts(asset), Some((8, 15)));
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

#[test]
fn coins_split_into_dex_and_symbol() {
    assert_eq!(split_coin("xyz:TSLA"), Some(("xyz", "TSLA")));
    assert_eq!(split_coin("para:10Y"), Some(("para", "10Y")));
    assert_eq!(split_coin("BTC"), None);
    assert_eq!(split_coin(":TSLA"), None);
    assert_eq!(split_coin("xyz:"), None);
    assert_eq!(coin("xyz", "TSLA"), "xyz:TSLA");
    assert_eq!(asset_parts(Asset::from_id(219_999)), Some((11, 9999)));
    assert_eq!(asset_parts(Asset::from_id(10_107)), None);
    assert_eq!(asset_parts(Asset::from_id(100_012_100)), None, "an outcome");
}

/// `para` clamps its three rates perps, the only clamps on mainnet; the docs
/// allow 0 to 0.01.
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

/// The caps `perpDexLimits` lists for `xyz` are the ones `perpDexs` streams
/// its coins toward: two reads that agree, all 123 of them.
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

#[test]
fn the_deploy_auction_ended_on_mainnet_and_ran_on_testnet() {
    let ended: DeployAuction = serde_json::from_str(AUCTION_MAINNET).unwrap();
    assert_eq!((ended.start_time_seconds, ended.duration_seconds), (1_790_398_800, 31 * 3600));
    assert_eq!((ended.current_gas, ended.end_gas), (None, Some(dec("541.69363865"))));
    assert_eq!(ended.start_gas, dec("1109.77867352"));
    assert!(!ended.is_running(ended.ends_at()));
    let running: DeployAuction = serde_json::from_str(AUCTION_TESTNET).unwrap();
    assert_eq!(
        (running.start_gas, running.current_gas, running.end_gas),
        (dec("1000"), Some(dec("527.03134408")), None)
    );
    assert!(running.is_running(running.start_time_seconds));
}

/// `metaAndAssetCtxs` with `dex: "para"` gives the meta `allPerpMetas` lists
/// at para's place, one context per perp.
#[test]
fn a_dex_s_contexts_line_up_with_its_meta() {
    let para: DexMetaAndAssetCtxs = serde_json::from_str(PARA).unwrap();
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

/// Every HIP-3 perp's table starts at its `maxLeverage`: a flat id under 50
/// is that leverage, a listed one has it as its first tier.
#[test]
fn every_hip3_perp_s_table_starts_at_its_max_leverage() {
    let dexes = joined();
    let mut count = 0;
    for (_, _, meta) in dexes.dexes() {
        for perp in &meta.universe {
            let table = meta.table_of(perp).unwrap();
            assert_eq!(table.max_leverage_at(Dec::ZERO), Some(perp.max_leverage), "{}", perp.name);
            count += 1;
        }
    }
    assert_eq!(count, 294);
}

#[test]
fn leverage_on_a_hip3_perp_is_held_to_what_it_allows() {
    let dexes = joined();
    let (asset, tsla) = dexes.find("xyz:TSLA").unwrap();
    let set = update_leverage(asset, tsla, MarginMode::Cross, 20).unwrap();
    assert_eq!(set.leverage(), 20);
    assert_eq!(
        update_leverage(asset, tsla, MarginMode::Cross, 21),
        Err(LeverageError::TooHigh { name: "xyz:TSLA".to_owned(), max: 20, asked: 21 })
    );
    let (asset, avgo) = dexes.find("para:AVGO").unwrap();
    assert_eq!(update_leverage(asset, avgo, MarginMode::Cross, 2), Err(LeverageError::NoCross("para:AVGO".to_owned())));
    let (_, _, para) = dexes.dex("para").unwrap();
    let table = para.table_of(avgo).unwrap();
    assert!(update_leverage_for(asset, avgo, &table, MarginMode::Isolated, 10, dec("1000000")).is_ok());
}

#[test]
fn the_reads_name_their_dex() {
    assert_eq!(all_perp_metas_request(), r#"{"type":"allPerpMetas"}"#);
    assert_eq!(perp_dex_limits_request("xyz"), r#"{"type":"perpDexLimits","dex":"xyz"}"#);
    assert_eq!(perp_dex_status_request("xyz"), r#"{"type":"perpDexStatus","dex":"xyz"}"#);
    assert_eq!(perp_deploy_auction_request(), r#"{"type":"perpDeployAuctionStatus"}"#);
    let transport = Recorded::new(&[
        ("perpDexs", DEXES),
        ("allPerpMetas", METAS),
        ("perpDexLimits", LIMITS),
        ("perpDexStatus", STATUS),
        ("perpDeployAuctionStatus", AUCTION_MAINNET),
    ]);
    assert_eq!(block_on(dexes(&transport)).unwrap(), joined());
    assert_eq!(
        block_on(perp_dex_limits(&transport, "xyz")).unwrap().map(|limits| limits.coin_to_oi_cap.len()),
        Some(123)
    );
    assert_eq!(
        block_on(perp_dex_status(&transport, "xyz")).unwrap().map(|status| status.total_net_deposit),
        Some(dec("1091195769.8109180927"))
    );
    assert_eq!(block_on(perp_deploy_auction(&transport)).unwrap().end_gas, Some(dec("541.69363865")));
    assert_eq!(block_on(perp_dex_limits(&transport, "")).unwrap(), None, "the main dex has no limits: null");
    let sent = transport.sent();
    assert_eq!(sent[..2], [r#"{"type":"perpDexs"}"#, r#"{"type":"allPerpMetas"}"#], "the list, then the metas");
    assert_eq!(sent[5], r#"{"type":"perpDexLimits","dex":""}"#);
}

#[test]
fn a_misaligned_answer_is_unreadable() {
    let transport = Recorded::new(&[("perpDexs", DEXES), ("allPerpMetas", "[]")]);
    assert!(block_on(dexes(&transport)).is_err());
}
