//! What hip4-tui's builder notes say in words, as numbers: the unit, each market kind's cap, and
//! D15's share of it (88 of a perp's 100, 880 of spot's and an outcome's 1,000).

use hl_core::{AssetKind, BuilderFee, Dec};

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn tenths(tenths: u64) -> BuilderFee {
    BuilderFee::from_tenths(tenths)
}

#[test]
fn a_fee_is_tenths_of_a_basis_point_so_ten_is_one_and_a_thousand_one_percent() {
    assert_eq!(TENTHS_PER_BASIS_POINT, 10);
    assert_eq!(rate(tenths(10)), dec("0.0001"), "one basis point");
    assert_eq!(rate(tenths(1_000)), dec("0.01"), "one percent");
    assert_eq!(rate(tenths(880)), dec("0.0088"));
    assert_eq!(rate(BuilderFee::ZERO), Dec::ZERO);
}

#[test]
fn the_cap_is_a_tenth_of_a_percent_on_perps_and_one_percent_on_spot_and_outcomes() {
    let caps: Vec<(AssetKind, u64)> = schedule(100).unwrap().iter().map(|cap| (cap.kind, cap.cap.tenths())).collect();
    assert_eq!(
        caps,
        [(AssetKind::Perp, 100), (AssetKind::Spot, 1_000), (AssetKind::Hip3Perp, 100), (AssetKind::Outcome, 1_000)],
        "an outcome is a spot asset, so its ceiling is spot's"
    );
}

#[test]
fn every_order_carries_the_builder_at_eighty_eight_percent_of_its_cap() {
    assert_eq!(SHARE_OF_CAP, 88);
    let fees: Vec<(AssetKind, u64, u64)> =
        schedule(SHARE_OF_CAP).unwrap().iter().map(|cap| (cap.kind, cap.fee.tenths(), cap.share)).collect();
    assert_eq!(
        fees,
        [
            (AssetKind::Perp, 88, 88),
            (AssetKind::Spot, 880, 88),
            (AssetKind::Hip3Perp, 88, 88),
            (AssetKind::Outcome, 880, 88)
        ]
    );
    assert!(schedule(101).is_none(), "more than the cap is no share of it");
    let nothing = schedule(0).unwrap();
    assert!(nothing.iter().all(|cap| cap.fee == BuilderFee::ZERO));
}

#[test]
fn a_share_is_cut_down_to_a_whole_tenth_of_a_basis_point() {
    let one = schedule(1).unwrap();
    assert_eq!((one[0].fee.tenths(), one[1].fee.tenths()), (1, 10));
    let odd = schedule(33).unwrap();
    assert_eq!((odd[0].fee.tenths(), odd[3].fee.tenths()), (33, 330));
}

#[test]
fn a_fee_is_paid_on_the_whole_notional_of_every_fill_that_names_it() {
    assert_eq!(paid(tenths(880), dec("100")), Some(dec("0.88")));
    assert_eq!(paid(tenths(100), dec("1000")), Some(dec("1")), "a perp's cap on a thousand");
    assert_eq!(paid(tenths(1), dec("12.5")), Some(dec("0.000125")));
    assert_eq!(paid(BuilderFee::ZERO, dec("1000")), Some(Dec::ZERO));
}

#[test]
fn how_much_of_its_cap_a_fee_takes_is_counted_in_hundredths_of_a_percent() {
    assert_eq!(share_of(tenths(880), AssetKind::Outcome), Some(8_800));
    assert_eq!(share_of(tenths(88), AssetKind::Perp), Some(8_800));
    assert_eq!(share_of(tenths(880), AssetKind::Perp), None, "over a perp's cap");
    assert_eq!(share_of(tenths(1), AssetKind::Spot), Some(10));
}
