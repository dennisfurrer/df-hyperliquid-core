//! Instants and days against Python's `datetime` (the milliseconds each figure is) and against
//! what hip4-tui's tests expected of chrono: the board's start date, yesterday's file and a
//! payout ledger's instants.

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
