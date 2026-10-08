//! Instants and minutes checked against Python's `datetime` in UTC, not against this module's
//! own calendar, and against what hip4-tui's tests expected of chrono: the board's start date,
//! yesterday's file and a payout ledger's instants. hl-hip4's stamp tests and hl-analytics' date
//! tests, together.

use super::*;

fn at(text: &str) -> At {
    At::parse(text).unwrap()
}

#[test]
fn an_instant_is_its_milliseconds_since_1970() {
    assert_eq!(at("1970-01-01T00:00:00Z").ms(), 0);
    assert_eq!(at("2026-09-17T12:00:00Z").ms(), 1_789_646_400_000);
    assert_eq!(at("2026-09-01T10:00:00.000Z").ms(), 1_788_256_800_000);
    assert_eq!(at("2024-02-29T23:59:59.999Z").ms(), 1_709_251_199_999, "a leap day");
    assert_eq!(at("1969-12-31T23:59:59Z").ms(), -1_000, "before 1970 counts down");
}

#[test]
fn an_offset_is_taken_back_to_utc() {
    assert_eq!(at("2026-09-01T12:30:00+02:00").ms(), 1_788_258_600_000);
    assert_eq!(at("2026-09-01T12:30:00+02:00"), at("2026-09-01T10:30:00Z"));
    assert_eq!(at("2026-09-01T05:30:00-05:00"), at("2026-09-01T10:30:00z"));
    assert_eq!(at("2026-09-01 10:30:00Z"), at("2026-09-01T10:30:00Z"), "a space for the T, as RFC 3339 allows");
}

#[test]
fn a_finer_fraction_is_cut_to_the_millisecond() {
    assert_eq!(at("2026-09-01T10:00:00.1234567Z").ms(), 1_788_256_800_123);
    assert_eq!(at("2026-09-01T10:00:00.5Z").ms(), 1_788_256_800_500);
}

#[test]
fn an_instant_is_written_with_milliseconds_and_a_z_as_the_ledger_holds_them() {
    assert_eq!(at("2026-09-01T10:00:00Z").to_string(), "2026-09-01T10:00:00.000Z");
    assert_eq!(at("2026-09-01T12:30:00.25+02:00").to_string(), "2026-09-01T10:30:00.250Z");
    assert_eq!(At::from_ms(-1_000).to_string(), "1969-12-31T23:59:59.000Z");
    let json = serde_json::to_string(&at("2026-09-01T10:01:01Z")).unwrap();
    assert_eq!(json, "\"2026-09-01T10:01:01.000Z\"");
    assert_eq!(serde_json::from_str::<At>(&json).unwrap(), at("2026-09-01T10:01:01Z"));
}

#[test]
fn anything_that_is_not_an_instant_is_refused() {
    for bad in [
        "",
        "2026-09-01",
        "2026-09-01T10:00:00",
        "2026-13-01T10:00:00Z",
        "2026-02-29T10:00:00Z",
        "2026-09-31T10:00:00Z",
        "2026-09-01T24:00:00Z",
        "2026-09-01T10:60:00Z",
        "2026-09-01T10:00:00.Z",
        "2026-09-01T10:00:00+2:00",
        "26-09-01T10:00:00Z",
        "2026-09-01T10:00:00Zjunk",
    ] {
        assert_eq!(At::parse(bad), None, "{bad}");
    }
    assert!(serde_json::from_str::<At>("\"yesterday\"").is_err());
}

#[test]
fn a_day_is_written_both_ways_the_venue_and_its_providers_use() {
    let instant = at("2026-09-17T12:00:00Z");
    assert_eq!(instant.date(), "2026-09-17");
    assert_eq!(instant.compact(), "20260917");
    assert_eq!(At::from_ms(-1).date(), "1969-12-31");
}

#[test]
fn days_move_whole_days_and_months_keep_to_the_end_of_a_short_month() {
    assert_eq!(at("2026-09-17T12:00:00Z").plus_days(-363).date(), "2025-09-19");
    assert_eq!(at("2027-01-01T12:00:00Z").plus_days(-1).compact(), "20261231");
    assert_eq!(at("2026-01-31T08:00:00Z").plus_months(1), at("2026-02-28T08:00:00Z"));
    assert_eq!(at("2024-01-31T08:00:00Z").plus_months(1), at("2024-02-29T08:00:00Z"), "a leap year");
    assert_eq!(at("2026-11-15T00:00:00Z").plus_months(2), at("2027-01-15T00:00:00Z"));
    assert_eq!(at("2026-03-31T00:00:00Z").plus_months(-1), at("2026-02-28T00:00:00Z"));
    assert_eq!(at("2026-11-15T00:00:00Z").plus_months(-12), at("2025-11-15T00:00:00Z"));
}

#[test]
fn a_date_reads_back_to_the_day_it_names() {
    assert_eq!(civil(at("2026-09-17T23:59:59Z")), (2026, 9, 17));
    assert_eq!(civil(at("2000-02-29T00:00:00Z")), (2000, 2, 29));
    assert_eq!(civil(At::from_ms(0)), (1970, 1, 1));
}

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

/// hl-hip4's deploy checks read a `dateTime` and a `date` this way: minutes from 1970 in UTC,
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
