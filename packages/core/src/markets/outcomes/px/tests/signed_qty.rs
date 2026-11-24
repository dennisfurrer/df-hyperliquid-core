//! Holdings as the venue prints them, long or short.

use super::*;

#[test]
fn a_holding_may_be_short_and_still_reads_like_a_size() {
    for (text, contracts) in [("1355.0", 1355), ("-539.0", -539), ("0.0", 0), (" -7 ", -7)] {
        let held = SignedQty::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
        assert_eq!(
            (held.get(), held.to_wire(), held.to_string().parse()),
            (contracts, contracts.to_string(), Ok(held))
        );
    }
    assert!(matches!(SignedQty::parse("-0.5"), Err(BadSignedQty::Size(BadQty::Fractional(text))) if text == "-0.5"));
    assert!(matches!(SignedQty::parse("x"), Err(BadSignedQty::Size(BadQty::NotAnInteger(_)))));
    assert!(matches!(SignedQty::parse("-10000000000000000000"), Err(BadSignedQty::TooLarge(_))));
    assert_eq!(SignedQty::from_dec(Dec::from_int(-539)), Ok(SignedQty::new(-539)));
    assert!(matches!(SignedQty::from_dec(Dec::parse("0.5").unwrap()), Err(BadSignedQty::Size(_))));
}

#[test]
fn a_short_leg_is_a_magnitude_and_a_sign_and_saturates_rather_than_wraps() {
    let short = SignedQty::short(Qty::new(539));
    let long = SignedQty::long(Qty::new(539));
    assert_eq!((short.abs(), long.abs()), (Qty::new(539), Qty::new(539)));
    assert!(short.is_short() && !long.is_short() && !SignedQty::ZERO.is_short() && SignedQty::ZERO.is_zero());
    assert_eq!(long.saturating_add(short), SignedQty::ZERO);
    assert!(short < long, "holdings order numerically, not by magnitude");
    assert_eq!(SignedQty::new(i64::MIN).abs().get(), 1u64 << 63);
    assert_eq!(SignedQty::new(i64::MAX).saturating_add(long), SignedQty::new(i64::MAX));
    assert_eq!(SignedQty::long(Qty::new(u64::MAX)), SignedQty::new(i64::MAX));
    assert_eq!(SignedQty::short(Qty::new(u64::MAX)), SignedQty::new(i64::MIN));
}

#[test]
fn a_holding_takes_one_sign_at_most() {
    assert!(SignedQty::parse("-+5").is_err(), "two signs are refused");
    assert_eq!(SignedQty::parse("-5").map(|held| held.to_wire()), Ok("-5".to_owned()));
}
