//! Minutes checked against Python's `datetime` in UTC, not against this module's own calendar.

use super::*;

#[test]
fn a_stamp_is_a_utc_minute_written_two_ways() {
    let cases = [
        ("20261001-0000", "2026-10-01 00:00Z", 1_790_812_800_000_i64),
        ("20260815-1126", "2026-08-15 11:26Z", 1_786_793_160_000),
        ("20280229-1200", "2028-02-29 12:00Z", 1_835_438_400_000),
        ("19691231-2359", "1969-12-31 23:59Z", -60_000),
    ];
    for (wire, read, ms) in cases {
        let stamp = Stamp::parse(wire).unwrap();
        assert_eq!(stamp.to_string(), read, "{wire}");
        assert_eq!(stamp.ms(), ms, "{wire}");
        assert_eq!(stamp.minutes() * 60_000, ms, "{wire}");
        assert_eq!(stamp.wire(), wire, "{wire} round-trips");
        assert_eq!(Stamp::from_minutes(stamp.minutes()), stamp);
    }
    assert_eq!(Stamp::parse(" 20261001-0000 "), Stamp::parse("20261001-0000"), "spaces around it aside");
}

#[test]
fn a_stamp_that_is_no_minute_is_refused() {
    for bad in ["20261001", "2026-10-01 00:00", "20261001-2400", "20261001-1260", "20260231-1200", "20250229-1200", ""]
    {
        assert_eq!(Stamp::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn a_stamp_has_passed_from_its_own_minute_on() {
    let stamp = Stamp::parse("20261001-0000").unwrap();
    assert!(!stamp.passed(1_790_812_799_999));
    assert!(stamp.passed(1_790_812_800_000), "the minute itself has passed");
    assert!(stamp.passed(u64::MAX));
    assert!(Stamp::parse("19691231-2359").unwrap().passed(0));
}

#[test]
fn a_day_or_a_minute_is_read_exactly_when_asked_for_exactly() {
    assert_eq!(Stamp::day("20261001"), Stamp::parse("20261001-0000"));
    assert_eq!(Stamp::exact(" 20261001-0000"), None, "a space is not a minute");
    for bad in ["2026101", "202610011", "20260231", "2026-10-1", "20261001-0000"] {
        assert_eq!(Stamp::day(bad), None, "{bad:?}");
    }
}

/// Outcome deploy checks read a `dateTime` and a `date` this way: minutes from 1970 in UTC,
/// 2026-09-30 12:00 being 1_790_769_600_000 ms.
#[test]
fn a_stamp_counts_minutes_from_1970_in_utc() {
    let minutes = |stamp: Option<Stamp>| stamp.map(Stamp::minutes);
    assert_eq!(minutes(Stamp::exact("19700101-0000")), Some(0));
    assert_eq!(minutes(Stamp::exact("19691231-2359")), Some(-1));
    assert_eq!(minutes(Stamp::exact("20260930-1200")), Some(1_790_769_600_000 / 60_000));
    assert_eq!(minutes(Stamp::day("20260930")), Some(29_845_440));
    assert_eq!(minutes(Stamp::day("20000301")), Some(11_017 * 1440));
    assert_eq!(Stamp::exact("20260930"), None);
    assert_eq!(Stamp::day("20260930-1200"), None);
}
