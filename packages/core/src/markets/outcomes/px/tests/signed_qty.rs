//! Holdings as the venue prints them, long or short.

use super::*;

#[test]
fn a_holding_may_be_short_and_still_reads_like_a_size() {
    assert_eq!(SignedQty::parse("1355.0"), Ok(SignedQty::new(1355)));
    assert_eq!(SignedQty::parse("-539.0"), Ok(SignedQty::new(-539)));
    assert_eq!(SignedQty::parse("0.0"), Ok(SignedQty::ZERO));
    assert_eq!(SignedQty::parse(" -7 ").unwrap().to_wire(), "-7");
    assert!(matches!(SignedQty::parse("-0.5"), Err(BadSignedQty::Size(BadQty::Fractional(_)))));
    assert!(matches!(SignedQty::parse("x"), Err(BadSignedQty::Size(BadQty::NotAnInteger(_)))));
    assert!(matches!(SignedQty::parse("-10000000000000000000"), Err(BadSignedQty::TooLarge(_))));
}

#[test]
fn a_short_leg_is_a_magnitude_and_a_sign_when_asked() {
    let short = SignedQty::short(Qty::new(539));
    let long = SignedQty::long(Qty::new(539));
    assert_eq!(short.abs(), Qty::new(539));
    assert_eq!(long.abs(), short.abs());
    assert!(short.is_short() && !long.is_short());
    assert_eq!(long.saturating_add(short), SignedQty::ZERO);
    assert!(short < long, "holdings order numerically, not by magnitude");
    assert_eq!(SignedQty::new(i64::MIN).abs().get(), 1u64 << 63);
    assert_eq!(SignedQty::new(i64::MAX).saturating_add(long), SignedQty::new(i64::MAX));
}
