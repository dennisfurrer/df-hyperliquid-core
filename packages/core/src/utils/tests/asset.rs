//! Outcome encoding: `#12100` (outcome 1210, Yes) is asset 100012100.

use super::*;

fn id(asset: Option<Asset>) -> u32 {
    asset.unwrap().id()
}

#[test]
fn each_kind_counts_from_its_own_base() {
    assert_eq!(id(Asset::perp(0)), 0);
    assert_eq!(id(Asset::perp(159)), 159);
    assert_eq!(id(Asset::hip3(1, 0)), 110_000);
    assert_eq!(id(Asset::hip3(1, 3)), 110_003);
    assert_eq!(id(Asset::hip3(10, 2)), 200_002);
    assert_eq!(id(Asset::spot(0)), 10_000);
    assert_eq!(id(Asset::spot(107)), 10_107);
    assert_eq!(id(Asset::outcome(1210, OutcomeSide::Yes)), 100_012_100);
    assert_eq!(id(Asset::outcome(1210, OutcomeSide::No)), 100_012_101);
    assert_eq!(id(Asset::outcome(516, OutcomeSide::Yes)), 100_005_160);
    assert_eq!(Asset::perp_on(0, 4), Asset::perp(4));
    assert_eq!(Asset::perp_on(2, 4), Asset::hip3(2, 4));
}

#[test]
fn an_index_past_its_range_is_refused_rather_than_naming_another_market() {
    assert_eq!(Asset::perp(10_000), None, "would be spot pair 0");
    assert_eq!(Asset::spot(90_000), None, "would be the first HIP-3 dex");
    assert_eq!(Asset::hip3(0, 1), None, "the main dex is not a HIP-3 dex");
    assert_eq!(Asset::hip3(1, 10_000), None, "would be the next dex's first asset");
    assert_eq!(Asset::hip3(9_990, 0), None, "would be an outcome");
    assert_eq!(id(Asset::hip3(9_989, 9_999)), 99_999_999);
    assert_eq!(Asset::hip3(u32::MAX, 0), None);
    assert_eq!(Asset::hip3(429_486, 9_999), None);
    assert_eq!(Asset::outcome(u32::MAX / 10, OutcomeSide::No), None);
    assert_eq!(id(Asset::outcome(419_496_719, OutcomeSide::No)), 4_294_967_191);
}

#[test]
fn an_id_says_what_it_names() {
    for (asset, kind) in [
        (0, AssetKind::Perp),
        (9_999, AssetKind::Perp),
        (10_000, AssetKind::Spot),
        (10_107, AssetKind::Spot),
        (99_999, AssetKind::Spot),
        (100_000, AssetKind::Hip3Perp),
        (110_001, AssetKind::Hip3Perp),
        (99_999_999, AssetKind::Hip3Perp),
        (100_000_000, AssetKind::Outcome),
        (100_012_100, AssetKind::Outcome),
        (100_012_101, AssetKind::Outcome),
    ] {
        assert_eq!(Asset::from_id(asset).kind(), kind, "asset {asset}");
    }
    assert_eq!(Asset::from_id(100_012_101).outcome_side(), Some((1210, OutcomeSide::No)));
    assert_eq!(Asset::from_id(100_012_102).outcome_side(), None, "no third side");
    assert_eq!(Asset::from_id(110_001).outcome_side(), None);
}

#[test]
fn one_perp_leg_holds_an_action_to_the_perp_cap() {
    let assets = |ids: &[u32]| ids.iter().map(|&id| Asset::from_id(id)).collect::<Vec<_>>();
    assert_eq!(AssetKind::strictest(assets(&[10_107, 100_012_100, 110_001])), Some(AssetKind::Hip3Perp));
    assert_eq!(AssetKind::strictest(assets(&[100_012_100, 100_012_101])), Some(AssetKind::Outcome));
    assert_eq!(AssetKind::strictest(assets(&[])), None);
    let caps: Vec<u64> = AssetKind::ALL.iter().map(|kind| kind.builder_cap().tenths()).collect();
    assert_eq!(caps, [100, 1000, 100, 1000]);
}

#[test]
fn an_asset_goes_on_the_wire_as_its_plain_id() {
    assert_eq!(Value::from(Asset::from_id(110_003)), Value::Uint(110_003));
    assert_eq!(Asset::from_id(10_107).to_string(), "10107");
}
