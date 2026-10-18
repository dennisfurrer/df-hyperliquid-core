//! Each hint by its name, what it refuses in words, and when a `dateTime` may be.

use super::*;
use crate::deploy::outcomes::actions::support::NOW_MS;

/// Each hint by the name `outcomeTemplates` gives it; one this build does not know is kept by its
/// name and checked as free text, which still keeps the rules every value keeps.
#[test]
fn each_hint_is_read_by_its_name_and_refuses_what_the_venue_refuses_in_words() {
    for name in ["dateTime", "date", "shortString", "hlPerp", "uInt", "uDecimal", "string"] {
        let hint = Hint::parse(name);
        assert!(!matches!(hint, Hint::Other(_)), "{name} is known");
        assert_eq!(hint.to_string(), name);
    }
    assert_eq!(Hint::parse("uPercent"), Hint::Other("uPercent".to_owned()));
    let date_time = "takes YYYYMMDD-HHMM in UTC";
    let date = "takes YYYYMMDD in UTC";
    let uint = "takes a whole number with no sign or leading zeros";
    let udecimal = "takes a decimal with no sign, no exponent and no leading or trailing zeros";
    let long_short = "x".repeat(11);
    let too_long = "x".repeat(101);
    let cases: Vec<(Hint, &str, &str, Option<&str>)> = vec![
        (Hint::DateTime, "time", "20261001-1200", None),
        (Hint::DateTime, "time", "20280229-1200", None),
        (Hint::DateTime, "time", "20000229-0000", None),
        (Hint::DateTime, "time", "20261231-2359", None),
        (Hint::DateTime, "time", "20261001", Some(date_time)),
        (Hint::DateTime, "time", "2026-10-01 12:00", Some(date_time)),
        (Hint::DateTime, "time", "20261001-2400", Some(date_time)),
        (Hint::DateTime, "time", "20261001-1260", Some(date_time)),
        (Hint::DateTime, "time", "20260231-1200", Some(date_time)),
        (Hint::DateTime, "time", "20250229-1200", Some(date_time)),
        (Hint::DateTime, "time", "21000229-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261301-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261000-1200", Some(date_time)),
        (Hint::DateTime, "time", " 20261001-1200", Some(date_time)),
        (Hint::DateTime, "time", "2026100a-1200", Some(date_time)),
        (Hint::DateTime, "time", "20261001+1200", Some(date_time)),
        (Hint::DateTime, "time", "20261001-120", Some(date_time)),
        (Hint::Date, "day", "20261001", None),
        (Hint::Date, "day", "20240229", None),
        (Hint::Date, "day", "20261001-1200", Some(date)),
        (Hint::Date, "day", "20261301", Some(date)),
        (Hint::Date, "day", "2026101", Some(date)),
        (Hint::Date, "day", "20230229", Some(date)),
        (Hint::ShortString, "shortNameA", "BRA", None),
        (Hint::ShortString, "shortNameA", "0123456789", None),
        (Hint::ShortString, "shortNameA", long_short.as_str(), Some("takes at most 10 characters")),
        (Hint::HlPerp, "perp", "BTC", None),
        (Hint::HlPerp, "perp", "", Some("needs a value")),
        (Hint::UInt, "seconds", "0", None),
        (Hint::UInt, "seconds", "60", None),
        (Hint::UInt, "seconds", "18446744073709551615", None),
        (Hint::UInt, "seconds", "18446744073709551616", Some("takes a whole number that fits in 64 bits")),
        (Hint::UInt, "seconds", "060", Some(uint)),
        (Hint::UInt, "seconds", "00", Some(uint)),
        (Hint::UInt, "seconds", "+60", Some(uint)),
        (Hint::UInt, "seconds", "-1", Some(uint)),
        (Hint::UInt, "seconds", "6e1", Some(uint)),
        (Hint::UInt, "seconds", "6.0", Some(uint)),
        (Hint::UInt, "seconds", " 60", Some(uint)),
        (Hint::UDecimal, "threshold", "0", None),
        (Hint::UDecimal, "threshold", "83114", None),
        (Hint::UDecimal, "threshold", "4460.9", None),
        (Hint::UDecimal, "threshold", "0.5", None),
        (Hint::UDecimal, "threshold", "0.05", None),
        (Hint::UDecimal, "threshold", "4460.90", Some(udecimal)),
        (Hint::UDecimal, "threshold", ".5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "05", Some(udecimal)),
        (Hint::UDecimal, "threshold", "00.5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1.", Some(udecimal)),
        (Hint::UDecimal, "threshold", "0.0", Some(udecimal)),
        (Hint::UDecimal, "threshold", "-1", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1e5", Some(udecimal)),
        (Hint::UDecimal, "threshold", "1,000", Some(udecimal)),
        (Hint::String, "priceDescription", "BTC-USDC perp mark", None),
        (Hint::String, "season", "2026/27", None),
        (Hint::String, "priceDescription", "a {b}", Some("cannot hold {, } or |")),
        (Hint::String, "priceDescription", "", Some("needs a value")),
        (Hint::String, "priceDescription", too_long.as_str(), Some("takes at most 100 characters")),
        (Hint::parse("uPercent"), "share", "anything, 0.5% or -3", None),
        (Hint::parse("uPercent"), "share", "a|b", Some("cannot hold {, } or |")),
    ];
    for (hint, keyword, value, expected) in cases {
        let got = hint.check(keyword, value).map_err(|bad| bad.to_string());
        let expected = expected.map(|words| format!("{keyword} {words}"));
        assert_eq!(got.err(), expected, "{hint} {value:?}");
    }
}

#[test]
fn a_date_time_is_after_now_and_within_366_days_of_it() {
    let at = |value: &str| Hint::DateTime.check_at("time", value, NOW_MS).map_err(|bad| bad.to_string());
    assert_eq!(at("20260930-1201"), Ok(()));
    assert_eq!(at("20261231-1200"), Ok(()));
    assert_eq!(at("20271001-1200"), Ok(()), "366 days to the minute");
    assert_eq!(at("20271001-1201"), Err("time is more than a year away".to_owned()));
    assert_eq!(at("20260930-1200"), Err("time has already passed".to_owned()), "now itself is not after now");
    assert_eq!(at("20260901-1200"), Err("time has already passed".to_owned()));
    assert_eq!(at("20261001"), Err("time takes YYYYMMDD-HHMM in UTC".to_owned()), "the shape is checked first");
    assert_eq!(Hint::DateTime.check("time", "20200101-0000"), Ok(()), "without a clock only the shape is checked");
    assert_eq!(Hint::Date.check_at("day", "20200101", NOW_MS), Ok(()), "a date has no rule on when");
}
