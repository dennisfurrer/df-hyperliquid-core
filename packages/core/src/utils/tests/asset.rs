//! Asset ids: each kind counted from its own base, never spilling into the next kind's, and an id
//! saying what it names. Outcome encoding: `#12100` (outcome 1210, Yes) is asset 100012100.

use super::*;

#[test]
fn each_kind_counts_from_its_own_base_and_never_spills_into_the_next() {
    for (asset, id, why) in [
        (Asset::perp(0), Some(0), ""),
        (Asset::perp(159), Some(159), ""),
        (Asset::hip3(1, 0), Some(110_000), ""),
        (Asset::hip3(1, 3), Some(110_003), ""),
        (Asset::hip3(10, 2), Some(200_002), ""),
        (Asset::spot(0), Some(10_000), ""),
        (Asset::spot(107), Some(10_107), ""),
        (Asset::outcome(1210, OutcomeSide::Yes), Some(100_012_100), ""),
        (Asset::outcome(1210, OutcomeSide::No), Some(100_012_101), ""),
        (Asset::outcome(516, OutcomeSide::Yes), Some(100_005_160), ""),
        (Asset::perp_on(0, 4), Some(4), "the main dex"),
        (Asset::perp_on(2, 4), Some(120_004), "a HIP-3 dex"),
        (Asset::perp(10_000), None, "would be spot pair 0"),
        (Asset::spot(90_000), None, "would be the first HIP-3 dex"),
        (Asset::hip3(0, 1), None, "the main dex is not a HIP-3 dex"),
        (Asset::hip3(1, 10_000), None, "would be the next dex's first asset"),
        (Asset::hip3(9_990, 0), None, "would be an outcome"),
        (Asset::hip3(9_989, 9_999), Some(99_999_999), "the last HIP-3 id"),
        (Asset::hip3(u32::MAX, 0), None, ""),
        (Asset::hip3(429_486, 9_999), None, ""),
        (Asset::outcome(u32::MAX / 10, OutcomeSide::No), None, ""),
        (Asset::outcome(419_496_719, OutcomeSide::No), Some(4_294_967_191), "the last outcome id"),
    ] {
        assert_eq!(asset.map(Asset::id), id, "{id:?} {why}");
    }
}

/// An id goes on the wire as itself, and says what it names.
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
    assert_eq!(Value::from(Asset::from_id(110_003)), Value::Uint(110_003));
    assert_eq!(Asset::from_id(10_107).to_string(), "10107");
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
