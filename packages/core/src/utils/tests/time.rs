//! Minutes checked against Python's `datetime` in UTC, not against this module's own calendar.

use super::*;

/// Outcome deploy checks read a `dateTime` and a `date` this way: minutes from 1970 in UTC.
#[test]
fn a_stamp_is_a_utc_minute_counted_from_1970_and_written_two_ways() {
    let cases = [
        ("20261001-0000", "2026-10-01 00:00Z", 1_790_812_800_000_i64),
        ("20260815-1126", "2026-08-15 11:26Z", 1_786_793_160_000),
        ("20280229-1200", "2028-02-29 12:00Z", 1_835_438_400_000),
        ("19691231-2359", "1969-12-31 23:59Z", -60_000),
        ("19700101-0000", "1970-01-01 00:00Z", 0),
        ("20260930-1200", "2026-09-30 12:00Z", 1_790_769_600_000),
    ];
    for (wire, read, ms) in cases {
        let stamp = Stamp::parse(wire).unwrap();
        assert_eq!(Stamp::exact(wire), Some(stamp), "{wire}");
        assert_eq!(stamp.to_string(), read, "{wire}");
        assert_eq!(stamp.ms(), ms, "{wire}");
        assert_eq!(stamp.minutes() * 60_000, ms, "{wire}");
        assert_eq!(stamp.wire(), wire, "{wire} round-trips");
        assert_eq!(Stamp::from_minutes(stamp.minutes()), stamp);
    }
    assert_eq!(Stamp::parse(" 20261001-0000 "), Stamp::parse("20261001-0000"), "spaces around it aside");
    for (day, minutes) in
        [("20261001", 1_790_812_800_000 / 60_000), ("20260930", 29_845_440), ("20000301", 11_017 * 1440)]
    {
        assert_eq!(Stamp::day(day).map(Stamp::minutes), Some(minutes), "{day} at midnight");
    }
}

#[test]
fn a_stamp_that_is_no_minute_or_no_day_is_refused() {
    for bad in ["20261001", "2026-10-01 00:00", "20261001-2400", "20261001-1260", "20260231-1200", "20250229-1200", ""]
    {
        assert_eq!(Stamp::parse(bad), None, "{bad:?}");
    }
    assert_eq!(Stamp::exact(" 20261001-0000"), None, "a space is not a minute");
    for bad in ["2026101", "202610011", "20260231", "2026-10-1", "20261001-0000", "20260930-1200"] {
        assert_eq!(Stamp::day(bad), None, "{bad:?}");
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
