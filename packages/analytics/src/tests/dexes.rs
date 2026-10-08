//! hip4-tui's `net/src/tests/perpdex.rs` and `tui/src/hip3/tests/mod.rs`, against the mainnet
//! answers they were written for (2026-09-26 and 27, stored as sent): `perpDexs`, `allPerpMetas`
//! and `metaAndAssetCtxs` for `para`. The counts, ranges and sums no hip4-tui test names were
//! taken from the same files with Python's `json` and `decimal`.

use hl_core::{Address, Dec};
use hl_hip3::{DexMeta, DexMetaAndAssetCtxs, Dexes, PerpDexs};

use super::*;

const DEXES: &str = include_str!("../../fixtures/perp_dexs_mainnet.json");
const METAS: &str = include_str!("../../fixtures/all_perp_metas_mainnet.json");
const PARA: &str = include_str!("../../fixtures/meta_and_asset_ctxs_para_mainnet.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn mainnet() -> Dexes {
    let list: PerpDexs = serde_json::from_str(DEXES).unwrap();
    let metas: Vec<DexMeta> = serde_json::from_str(METAS).unwrap();
    Dexes::new(list, metas).unwrap()
}

fn named<'a>(setups: &'a [Setup], name: &str) -> &'a Setup {
    setups.iter().find(|one| one.name == name).unwrap()
}

#[test]
fn every_hip3_dex_is_set_out_and_the_native_one_is_left_out() {
    let setups = setups(&mainnet());
    let names: Vec<&str> = setups.iter().map(|one| one.name.as_str()).collect();
    assert_eq!(names, ["xyz", "flx", "vntl", "hyna", "km", "abcd", "cash", "para", "mkts", "io"]);
    assert_eq!(setups[1].full_name, "Felix Exchange");
    assert_eq!(setups.iter().map(|one| one.place).collect::<Vec<_>>(), (1..=10).collect::<Vec<u32>>());
}

#[test]
fn a_dex_names_its_deployer_oracle_updater_and_fee_recipient_where_it_has_them() {
    let setups = setups(&mainnet());
    let flx = named(&setups, "flx");
    assert_eq!(flx.deployer, Some(address("0x2fab552502a6d45920d5741a2f3ebf4c35536352")));
    assert_eq!(flx.oracle_updater, Some(address("0x94757f8dcb4bf73b850195660e959d1105cfedd5")));
    assert_eq!(flx.fee_recipient, Some(address("0xe2872b5ae7dcbba40cc4510d08c8bbea95b42d43")));
    assert_eq!(named(&setups, "xyz").oracle_updater, None, "null is kept as none");
    let abcd = named(&setups, "abcd");
    assert!(abcd.delegates.is_empty());
    assert_eq!(abcd.fee_recipient, None);
}

#[test]
fn a_dexs_delegates_are_its_addresses_each_with_what_it_may_send() {
    let dexes = mainnet();
    let (_, xyz, _) = dexes.dex("xyz").unwrap();
    let granted = delegates(xyz);
    let actions: Vec<usize> = granted.iter().map(|one| one.actions.len()).collect();
    assert_eq!(actions, [10, 9, 8, 1], "the one trusted with most first");
    let listed: usize = xyz.sub_deployers.iter().map(|(_, executors)| executors.len()).sum();
    assert_eq!(actions.iter().sum::<usize>(), listed, "every grant lands on one address");
    assert_eq!(xyz.sub_deployers.len(), 11);
    assert_eq!(named(&setups(&dexes), "xyz").delegates, granted);
}

#[test]
fn assets_are_counted_live_and_delisted_with_their_fee_scales() {
    let setups = setups(&mainnet());
    let flx = named(&setups, "flx");
    assert_eq!((flx.assets, flx.live, flx.delisted), (16, 0, 16));
    assert_eq!(flx.collateral, Some(360), "USDH");
    assert_eq!(flx.fee_scales, None, "no live asset carries one");
    let para = named(&setups, "para");
    assert_eq!((para.assets, para.live, para.delisted), (36, 29, 7));
    assert_eq!(para.fee_scales, Some((dec("0.5"), dec("1.0"))), "set an asset at a time, so a range");
    let xyz = named(&setups, "xyz");
    assert_eq!((xyz.assets, xyz.live, xyz.delisted, xyz.fee_scales), (126, 109, 17, Some((Dec::ONE, Dec::ONE))));
    assert_eq!(named(&setups, "hyna").collateral, Some(235), "USDE");
}

#[test]
fn an_asset_margins_the_way_its_deployer_set_it() {
    let dexes = mainnet();
    let (_, _, flx) = dexes.dex("flx").unwrap();
    let tsla = &flx.universe[0];
    assert_eq!(tsla.name, "flx:TSLA");
    assert!(tsla.is_delisted);
    assert_eq!(margin(tsla), "cross");
    assert_eq!(tsla.deployer_fee_scale, Some(dec("1.0")));
    assert_eq!(tsla.growth_mode.as_deref(), Some("enabled"));
    let crcl = &flx.universe[2];
    assert_eq!(margin(crcl), "strictIsolated");
    assert!(crcl.only_isolated);
}

#[test]
fn streaming_caps_and_funding_settings_are_counted_a_coin_at_a_time() {
    let setups = setups(&mainnet());
    let xyz = named(&setups, "xyz");
    assert_eq!((xyz.streaming_caps, xyz.funding_multipliers, xyz.funding_rates, xyz.funding_clamps), (123, 123, 4, 0));
    let km = named(&setups, "km");
    assert_eq!((km.streaming_caps, km.funding_multipliers, km.funding_rates), (22, 0, 22));
    assert_eq!(named(&setups, "para").funding_clamps, 3);
}

#[test]
fn a_deployer_of_two_dexes_is_one_deployer_with_both() {
    let deployed = deployers(&mainnet());
    assert_eq!(deployed.len(), 9, "ten dexes, nine deployers");
    let (who, dexes) = &deployed[0];
    assert_eq!(*who, address("0x71f0019cc7fa79e4f42587fb7b9a817d8d2429ec"), "the deployer of most first");
    assert_eq!(dexes, &["km", "mkts"]);
    assert!(deployed[1..].iter().all(|(_, dexes)| dexes.len() == 1));
}

#[test]
fn open_interest_is_each_assets_size_at_its_mark_and_volume_the_days_notional() {
    let para: DexMetaAndAssetCtxs = serde_json::from_str(PARA).unwrap();
    let activity = activity(&para);
    assert_eq!(activity.open_interest, dec("17451484.683234"));
    assert_eq!(activity.volume, dec("2730449.4484000011"));
    assert_eq!(activity.assets, 36);
}
